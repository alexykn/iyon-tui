use std::sync::Weak;

use super::*;

struct LatchRelease(Option<std::sync::mpsc::Sender<()>>);

impl LatchRelease {
    fn release(&mut self) {
        if let Some(release) = self.0.take() {
            let _ = release.send(());
        }
    }
}

impl Drop for LatchRelease {
    fn drop(&mut self) {
        self.release();
    }
}

#[test]
fn captured_measurement_refinement_keeps_the_candidate_source_frontier() {
    let source_registry = ContentSourceRegistry::new();
    let source = source_registry.create(TextSourceKind::Stream).unwrap();
    source.append_utf8(b"alpha beta\n", &[], &[]).unwrap();
    let source_revision = source.snapshot().unwrap().revision;
    let mut registry = ContentHostRegistry::new(source_registry);
    let port = registry
        .create_port(Weak::new(), ContentFamily::Text)
        .unwrap();
    let connector = registry
        .connect(
            &port.record,
            &source,
            HostContentFunnel::plain(TextWrapMode::Word),
        )
        .unwrap();
    {
        let mut state = port.record.lock().unwrap();
        state.desired_mounted = true;
        state.visible_mounted = true;
        state.desired_connector = Some(connector.id());
        state.visible_connector = Some(connector.id());
    }
    {
        let record = registry.connectors.get(&connector.id()).unwrap();
        let mut state = record.lock().unwrap();
        state.requested = true;
        state.visible = true;
    }
    let (entered, release) = registry.install_projection_latch_for_test();
    let mut release = LatchRelease(Some(release));
    registry.begin_projection_candidate();
    registry
        .prepare_connector_projection(connector.id(), 20)
        .unwrap();
    entered
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("content worker entered latch");
    release.release();
    registry.wait_for_projection_jobs_for_test();
    registry.clear_projection_latch_for_test();
    registry.begin_projection_candidate();
    registry
        .prepare_connector_projection(connector.id(), 20)
        .unwrap();
    registry.promote_candidate_projection(connector.id());
    registry.end_candidate();
    registry.begin_projection_candidate();
    let captured = registry
        .capture_measurement(port.id(), 20, crate::presentation::ContentWidthRule::Fit)
        .unwrap();
    assert!(
        captured.min_content.width < captured.max_content.width,
        "word wrapping must expose a smaller min-content width"
    );
    let narrow = crate::text::TerminalTextProjector::new(captured.terminal_policy.clone())
        .project_contents(
            captured
                .semantic_contents
                .as_deref()
                .expect("captured semantic content product"),
            crate::text::TerminalConstraints::definite(5),
        )
        .expect("narrow terminal product")
        .size()
        .height();
    assert!(
        narrow >= 2,
        "known narrow width must recompute wrapped height"
    );
    let captured_source = registry
        .candidate_content_captures
        .get(&captured.capture_id)
        .and_then(|capture| match &capture.candidate {
            CapturedCandidate::Prepared(binding) => Some(binding.source_snapshot.clone()),
            CapturedCandidate::None | CapturedCandidate::Failed { .. } => None,
        })
        .expect("captured source snapshot");
    source.append_utf8(b"newest source\n", &[], &[]).unwrap();
    registry
        .prepare_connector_projection_async(connector.id(), 5, Some(&captured_source))
        .unwrap();
    registry.wait_for_projection_jobs_for_test();
    let refined = registry
        .refine_captured_measurement(
            port.id(),
            captured.capture_id,
            5,
            crate::presentation::ContentWidthRule::Fill,
        )
        .unwrap();
    assert_eq!(refined.capture_id, captured.capture_id);
    let projection = registry
        .projection_for_measurement(connector.id(), refined.measurement)
        .expect("captured refined projection");
    assert_eq!(projection.key.source_revision, source_revision);
    assert_eq!(registry.candidate_source_snapshots.borrow().len(), 1);
    registry.abort_candidate();
}

#[test]
fn final_width_failure_uses_the_captured_confirmed_a_product() {
    let source_registry = ContentSourceRegistry::new();
    let source_a = source_registry.create(TextSourceKind::Stream).unwrap();
    let source_b = source_registry.create(TextSourceKind::Stream).unwrap();
    source_a.append_utf8(b"confirmed A\n", &[], &[]).unwrap();
    source_b.append_utf8(b"candidate B\n", &[], &[]).unwrap();
    let a_revision = source_a.snapshot().unwrap().revision;
    let mut registry = ContentHostRegistry::new(source_registry);
    let port = registry
        .create_port(Weak::new(), ContentFamily::Text)
        .unwrap();
    let connector_a = registry
        .connect(
            &port.record,
            &source_a,
            HostContentFunnel::plain(TextWrapMode::Word),
        )
        .unwrap();
    let connector_b = registry
        .connect(
            &port.record,
            &source_b,
            HostContentFunnel::plain(TextWrapMode::Word),
        )
        .unwrap();
    {
        let mut state = port.record.lock().unwrap();
        state.desired_mounted = true;
        state.visible_mounted = true;
        state.desired_connector = Some(connector_a.id());
        state.visible_connector = Some(connector_a.id());
    }
    registry
        .connectors
        .get(&connector_a.id())
        .unwrap()
        .lock()
        .unwrap()
        .visible = true;
    registry.begin_projection_candidate();
    registry
        .prepare_connector_projection(connector_a.id(), 20)
        .unwrap();
    registry.wait_for_projection_jobs_for_test();
    registry.begin_projection_candidate();
    let a_measurement = registry
        .prepare_connector_projection(connector_a.id(), 20)
        .unwrap();
    registry.promote_candidate_projection(connector_a.id());
    registry.end_candidate();
    source_a.append_utf8(b"newest A\n", &[], &[]).unwrap();
    {
        let mut state = port.record.lock().unwrap();
        state.desired_connector = Some(connector_b.id());
    }
    registry
        .connectors
        .get(&connector_b.id())
        .unwrap()
        .lock()
        .unwrap()
        .requested = true;
    registry.begin_projection_candidate();
    registry
        .prepare_connector_projection(connector_b.id(), 20)
        .unwrap();
    registry.wait_for_projection_jobs_for_test();
    let captured = registry
        .capture_measurement(port.id(), 20, crate::presentation::ContentWidthRule::Fit)
        .unwrap();
    let confirmed = registry
        .candidate_content_captures
        .get(&captured.capture_id)
        .and_then(|capture| capture.confirmed.as_ref())
        .map(|capture| &capture.product)
        .expect("captured confirmed A product");
    assert_eq!(confirmed.identity, a_measurement.projection_identity);
    assert_eq!(confirmed.key.source_revision, a_revision);
    let failed_key = registry
        .connector_projection_key(connector_b.id(), 5)
        .unwrap();
    {
        let record = registry.connectors.get(&connector_b.id()).unwrap();
        let mut state = record.lock().unwrap();
        state.error = Some(ContentConnectorError {
            code: "PROJECTION_FAILED".to_owned(),
            diagnostic: "final-width failure".to_owned(),
        });
        state.projection_failure_key = Some(failed_key);
    }
    let refined = registry
        .refine_captured_measurement(
            port.id(),
            captured.capture_id,
            5,
            crate::presentation::ContentWidthRule::Fill,
        )
        .unwrap();
    assert_eq!(refined.measurement.connector_id, Some(connector_a.id()));
    let product = registry
        .projection_for_measurement(connector_a.id(), refined.measurement)
        .expect("confirmed A product");
    assert_eq!(product.key.source_revision, a_revision);
    registry.abort_candidate();
}

#[test]
fn same_source_b_capture_cannot_replace_confirmed_a_frontier() {
    let source_registry = ContentSourceRegistry::new();
    let source = source_registry.create(TextSourceKind::Stream).unwrap();
    source.append_utf8(b"A rev1\n", &[], &[]).unwrap();
    let a_revision = source.snapshot().unwrap().revision;
    let mut registry = ContentHostRegistry::new(source_registry);
    let port = registry
        .create_port(Weak::new(), ContentFamily::Text)
        .unwrap();
    let connector_a = registry
        .connect(
            &port.record,
            &source,
            HostContentFunnel::plain(TextWrapMode::Word),
        )
        .unwrap();
    let connector_b = registry
        .connect(
            &port.record,
            &source,
            HostContentFunnel::plain(TextWrapMode::Word),
        )
        .unwrap();
    {
        let mut state = port.record.lock().unwrap();
        state.desired_mounted = true;
        state.visible_mounted = true;
        state.desired_connector = Some(connector_a.id());
        state.visible_connector = Some(connector_a.id());
    }
    {
        let record = registry.connectors.get(&connector_a.id()).unwrap();
        let mut state = record.lock().unwrap();
        state.requested = true;
        state.visible = true;
    }
    registry.begin_projection_candidate();
    registry
        .prepare_connector_projection(connector_a.id(), 20)
        .unwrap();
    registry.wait_for_projection_jobs_for_test();
    registry.begin_projection_candidate();
    registry
        .prepare_connector_projection(connector_a.id(), 20)
        .unwrap();
    registry.promote_candidate_projection(connector_a.id());
    registry.end_candidate();
    source.append_utf8(b"B rev2\n", &[], &[]).unwrap();
    {
        let mut state = port.record.lock().unwrap();
        state.desired_connector = Some(connector_b.id());
    }
    registry
        .connectors
        .get(&connector_b.id())
        .unwrap()
        .lock()
        .unwrap()
        .requested = true;
    registry.begin_projection_candidate();
    registry
        .prepare_connector_projection(connector_b.id(), 20)
        .unwrap();
    registry.wait_for_projection_jobs_for_test();
    let captured = registry
        .capture_measurement(port.id(), 20, crate::presentation::ContentWidthRule::Fit)
        .unwrap();
    let candidate = registry
        .candidate_content_captures
        .get(&captured.capture_id)
        .expect("candidate capture");
    assert_eq!(
        match &candidate.candidate {
            CapturedCandidate::Prepared(capture) => &capture.product,
            CapturedCandidate::None | CapturedCandidate::Failed { .. } => panic!("B product"),
        }
        .key
        .source_revision,
        a_revision + 1
    );
    assert_eq!(
        candidate
            .confirmed
            .as_ref()
            .map(|capture| &capture.product)
            .expect("A product")
            .key
            .source_revision,
        a_revision
    );
    let failed_key = registry
        .connector_projection_key(connector_b.id(), 5)
        .unwrap();
    {
        let record = registry.connectors.get(&connector_b.id()).unwrap();
        let mut state = record.lock().unwrap();
        state.error = Some(ContentConnectorError {
            code: "PROJECTION_FAILED".to_owned(),
            diagnostic: "same-source final-width failure".to_owned(),
        });
        state.projection_failure_key = Some(failed_key);
    }
    let refined = registry
        .refine_captured_measurement(
            port.id(),
            captured.capture_id,
            5,
            crate::presentation::ContentWidthRule::Fill,
        )
        .unwrap();
    let product = registry
        .projection_for_measurement(connector_a.id(), refined.measurement)
        .expect("A fallback product");
    assert_eq!(product.key.source_revision, a_revision);
    registry.abort_candidate();
}

#[test]
fn continuous_source_appends_coalesce_while_projection_is_pending() {
    let source_registry = ContentSourceRegistry::new();
    let source = source_registry.create(TextSourceKind::Stream).unwrap();
    source.append_utf8(b"prefix\n", &[], &[]).unwrap();
    let mut registry = ContentHostRegistry::new(source_registry);
    let port = registry
        .create_port(Weak::new(), ContentFamily::Text)
        .unwrap();
    let connector = registry
        .connect(
            &port.record,
            &source,
            HostContentFunnel::plain(TextWrapMode::Word),
        )
        .unwrap();
    {
        let mut port_state = port.record.lock().unwrap();
        port_state.desired_mounted = true;
        port_state.visible_mounted = true;
        port_state.desired_connector = Some(connector.id());
        port_state.visible_connector = Some(connector.id());
    }
    {
        let connector_state = registry.connectors.get(&connector.id()).unwrap();
        let mut connector_state = connector_state.lock().unwrap();
        connector_state.requested = true;
        connector_state.visible = true;
    }
    let (entered, release) = registry.install_projection_latch_for_test();
    let mut release = LatchRelease(Some(release));
    registry.begin_projection_candidate();
    registry
        .prepare_connector_projection(connector.id(), 20)
        .unwrap();
    entered
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("projection worker entered latch");
    for _ in 0..64 {
        source.append_utf8(b"append\n", &[], &[]).unwrap();
        registry
            .prepare_connector_projection(connector.id(), 20)
            .unwrap();
        assert_eq!(registry.pending_content_projections.len(), 1);
    }
    let latest = source.snapshot().unwrap();
    release.release();
    registry.wait_for_projection_jobs_for_test();
    registry.clear_projection_latch_for_test();
    // A retained physical frame still owns the old compatible product when
    // the exact newer result arrives. It must not shadow that cache entry.
    {
        let mut state = connector.record.lock().unwrap();
        state.committed_projection = state
            .projection_cache
            .front()
            .map(|(_, product)| Arc::clone(product));
    }
    registry.begin_projection_candidate();
    registry
        .prepare_connector_projection(connector.id(), 20)
        .unwrap();
    registry.wait_for_projection_jobs_for_test();
    registry.begin_projection_candidate();
    let measurement = registry
        .prepare_connector_projection(connector.id(), 20)
        .unwrap();
    let projection = registry
        .projection_for_measurement(connector.id(), measurement)
        .expect("latest coalesced projection");
    assert_eq!(projection.source_snapshot.source_end, latest.source_end);
}

#[test]
fn prepared_content_commit_preserves_newer_requested_selection() {
    let source_registry = ContentSourceRegistry::new();
    let source = source_registry.create(TextSourceKind::Stream).unwrap();
    source.append_utf8(b"old\n", &[], &[]).unwrap();
    let mut registry = ContentHostRegistry::new(source_registry);
    let port = registry
        .create_port(Weak::new(), ContentFamily::Text)
        .unwrap();
    let first = registry
        .connect(
            &port.record,
            &source,
            HostContentFunnel::plain(TextWrapMode::Word),
        )
        .unwrap();
    let second = registry
        .connect(
            &port.record,
            &source,
            HostContentFunnel::plain(TextWrapMode::Word),
        )
        .unwrap();
    {
        let mut state = port.record.lock().unwrap();
        state.desired_mounted = true;
        state.visible_mounted = true;
        state.desired_connector = Some(first.id());
        state.visible_connector = Some(first.id());
    }
    {
        let record = registry.connectors.get(&first.id()).unwrap();
        let mut state = record.lock().unwrap();
        state.requested = true;
        state.visible = true;
    }

    registry.begin_projection_candidate();
    registry
        .prepare_connector_projection(first.id(), 20)
        .unwrap();
    registry.candidate_binding_changes.insert(port.id());
    let plan = registry.prepare_content_commit().unwrap();
    registry.begin_prepared_candidate(&plan);

    // A newer request is accepted while the old immutable candidate remains
    // owned by its delayed receipt. Committing the old plan must not consume
    // or rewrite the new desired selection.
    assert!(
        registry
            .request_activation(second.id(), &Weak::new())
            .unwrap()
    );
    registry.commit_prepared(&plan).unwrap();
    registry.end_candidate();

    let port_state = port.record.lock().unwrap();
    assert_eq!(port_state.visible_connector, Some(first.id()));
    assert_eq!(port_state.desired_connector, Some(second.id()));
    drop(port_state);
    assert!(
        registry
            .connectors
            .get(&first.id())
            .unwrap()
            .lock()
            .unwrap()
            .visible
    );
    assert!(
        registry
            .connectors
            .get(&second.id())
            .unwrap()
            .lock()
            .unwrap()
            .requested
    );
}
