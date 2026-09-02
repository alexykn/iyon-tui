use std::{cell::RefCell, rc::Rc, time::Instant};

use super::*;
use crate::stream::{StreamOffset, StreamRange};

fn range(start: u64, end: u64) -> StreamRange {
    StreamRange::new(StreamOffset::new(start), StreamOffset::new(end))
}

fn complete<T>(stable: u64, sealed: bool) -> ProjectionBuilder<T> {
    ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(stable),
        StreamOffset::new(4),
        sealed,
    )
}

fn smooth_input(stable: u64, end: u64, sealed: bool, weights: &[usize]) -> Projection<i32> {
    let mut builder = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(stable),
        StreamOffset::new(end),
        sealed,
    );
    let mut cursor = 0;
    for &weight in weights {
        let next = cursor + weight as u64;
        builder = builder.emit_many(range(cursor, next), (0..weight).map(|value| value as i32));
        cursor = next;
    }
    builder.finish().unwrap()
}

#[test]
fn smooth_only_publishes_upstream_stable_spans_and_seals_to_identity() {
    let input = smooth_input(2, 4, false, &[1, 1, 1, 1]);
    let mut smooth = Smooth::default();
    let output = smooth.project(&input).unwrap();
    assert_eq!(output.source_end(), StreamOffset::new(1));
    assert_eq!(output.stable_through(), output.source_end());

    let sealed = smooth_input(4, 4, true, &[1, 1, 1, 1]);
    assert_eq!(smooth.project(&sealed).unwrap(), sealed);
    assert_eq!(smooth.next_wakeup(), None);
}

#[test]
fn smooth_preserves_atomic_spans_and_accumulates_credit() {
    let input = smooth_input(11, 11, false, &[1, 10]);
    let config =
        SmoothConfig::try_from_parts(std::time::Duration::from_millis(1), 1.0, 3000.0, 3000.0)
            .unwrap();
    let mut smooth = Smooth::new(config);
    assert_eq!(
        smooth.project(&input).unwrap().source_end(),
        StreamOffset::new(1)
    );
    let t0 = std::time::Instant::now();
    assert!(!smooth.advance(t0));
    for index in 1..4 {
        let _ = smooth.advance(t0 + std::time::Duration::from_millis(index));
    }
    let _ = smooth.advance(t0 + std::time::Duration::from_millis(4));
    let _ = smooth.project(&input);
    for index in 5..11 {
        let _ = smooth.advance(t0 + std::time::Duration::from_millis(index));
    }
    let _ = smooth.advance(t0 + std::time::Duration::from_millis(12));
    assert!(smooth.next_wakeup().is_none());
    assert_eq!(
        smooth.project(&input).unwrap().source_end(),
        StreamOffset::new(11)
    );
}

#[test]
fn smooth_handles_elision_without_pacing_cost() {
    let input = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(3),
        StreamOffset::new(3),
        false,
    )
    .elide(range(0, 1))
    .emit(range(1, 3), 7)
    .finish()
    .unwrap();
    let mut smooth = Smooth::default();
    let output = smooth.project(&input).unwrap();
    assert_eq!(output.source_end(), StreamOffset::new(3));
    assert_eq!(output.spans().len(), 2);
}

#[test]
fn construction_requires_exact_nonempty_coverage_and_allows_elision() {
    assert!(
        ProjectionBuilder::<u8>::new(
            StreamOffset::ZERO,
            StreamOffset::ZERO,
            StreamOffset::ZERO,
            false
        )
        .finish()
        .is_ok()
    );
    assert!(
        complete::<u8>(0, false)
            .emit(range(0, 0), 1)
            .finish()
            .is_err()
    );
    assert!(
        complete::<u8>(0, false)
            .emit(range(1, 4), 1)
            .finish()
            .is_err()
    );
    assert!(
        complete::<u8>(0, false)
            .elide(range(0, 3))
            .emit(range(2, 4), 1)
            .finish()
            .is_err()
    );
    assert!(
        complete::<u8>(0, false)
            .elide(range(0, 2))
            .emit(range(2, 4), 1)
            .finish()
            .is_ok()
    );
}

#[test]
fn stable_frontier_must_be_a_span_boundary() {
    assert_eq!(
        complete::<u8>(1, false)
            .emit(range(0, 2), 1)
            .elide(range(2, 4))
            .finish(),
        Err(ProjectionValidationError::StableFrontierInsideSpan)
    );
    assert!(
        complete::<u8>(2, false)
            .emit(range(0, 2), 1)
            .elide(range(2, 4))
            .finish()
            .is_ok()
    );
    assert!(
        complete::<u8>(4, true)
            .emit(range(0, 4), 1)
            .finish()
            .is_ok()
    );
}

#[test]
fn transitions_freeze_values_and_segmentation_but_allow_tail_replacement() {
    let previous = complete::<u8>(2, false)
        .emit(range(0, 1), 1)
        .emit(range(1, 2), 2)
        .elide(range(2, 4))
        .finish()
        .unwrap();
    let next = complete::<u8>(4, true)
        .emit_many(range(0, 2), [1, 2])
        .emit_many(range(2, 4), [2, 3])
        .finish()
        .unwrap();
    assert_eq!(
        validate_projection_transition(&previous, &next),
        Err(ProjectionTransitionError::StablePrefixChanged)
    );

    let tail_replaced = complete::<u8>(2, false)
        .emit(range(0, 1), 1)
        .emit(range(1, 2), 2)
        .emit_many(range(2, 4), [2, 3])
        .finish()
        .unwrap();
    validate_projection_transition(&previous, &tail_replaced).unwrap();

    let changed = complete::<u8>(2, false)
        .emit(range(0, 1), 9)
        .emit(range(1, 2), 2)
        .elide(range(2, 4))
        .finish()
        .unwrap();
    assert_eq!(
        validate_projection_transition(&previous, &changed),
        Err(ProjectionTransitionError::StablePrefixChanged)
    );
}

#[test]
fn compaction_may_remove_only_stable_prefix() {
    let previous = complete::<u8>(2, false)
        .emit(range(0, 2), 1)
        .elide(range(2, 4))
        .finish()
        .unwrap();
    let compacted = ProjectionBuilder::new(
        StreamOffset::new(2),
        StreamOffset::new(2),
        StreamOffset::new(4),
        false,
    )
    .elide(range(2, 4))
    .finish()
    .unwrap();
    validate_projection_transition(&previous, &compacted).unwrap();

    let invalid = ProjectionBuilder::new(
        StreamOffset::new(3),
        StreamOffset::new(3),
        StreamOffset::new(4),
        false,
    )
    .elide(range(3, 4))
    .finish()
    .unwrap();
    assert_eq!(
        validate_projection_transition(&previous, &invalid),
        Err(ProjectionTransitionError::SourceBaseBeyondPreviousStability)
    );
}

#[test]
fn relation_allows_lagging_and_sealed_input() {
    let input = ProjectionBuilder::<u8>::new(
        StreamOffset::ZERO,
        StreamOffset::new(4),
        StreamOffset::new(4),
        true,
    )
    .emit(range(0, 4), 1)
    .finish()
    .unwrap();
    let output = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(2),
        StreamOffset::new(2),
        false,
    )
    .emit_many(range(0, 2), [1, 1])
    .finish()
    .unwrap();
    validate_projection_relation(&input, &output).unwrap();
    let final_output = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(4),
        StreamOffset::new(4),
        true,
    )
    .emit_many(range(0, 4), [1, 1])
    .finish()
    .unwrap();
    validate_projection_relation(&input, &final_output).unwrap();

    let base_mismatch = ProjectionBuilder::new(
        StreamOffset::new(1),
        StreamOffset::new(1),
        StreamOffset::new(2),
        false,
    )
    .emit(range(1, 2), 1)
    .finish()
    .unwrap();
    assert_eq!(
        validate_projection_relation(&input, &base_mismatch),
        Err(ProjectionRelationError::SourceBaseMismatch)
    );
    let beyond_end = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(4),
        StreamOffset::new(5),
        false,
    )
    .emit(range(0, 4), 1)
    .emit(range(4, 5), 1)
    .finish()
    .unwrap();
    assert_eq!(
        validate_projection_relation(&input, &beyond_end),
        Err(ProjectionRelationError::OutputEndBeyondInput)
    );
    let stability_input = ProjectionBuilder::<u8>::new(
        StreamOffset::ZERO,
        StreamOffset::new(2),
        StreamOffset::new(4),
        false,
    )
    .emit(range(0, 2), 1)
    .elide(range(2, 4))
    .finish()
    .unwrap();
    let beyond_stability = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(3),
        StreamOffset::new(4),
        false,
    )
    .emit(range(0, 3), 1)
    .elide(range(3, 4))
    .finish()
    .unwrap();
    assert_eq!(
        validate_projection_relation(&stability_input, &beyond_stability),
        Err(ProjectionRelationError::OutputStabilityBeyondInput)
    );
    let sealed_open_input = ProjectionBuilder::<u8>::new(
        StreamOffset::ZERO,
        StreamOffset::new(4),
        StreamOffset::new(4),
        false,
    )
    .emit(range(0, 4), 1)
    .finish()
    .unwrap();
    let sealed_output = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(4),
        StreamOffset::new(4),
        true,
    )
    .emit(range(0, 4), 1)
    .finish()
    .unwrap();
    assert_eq!(
        validate_projection_relation(&sealed_open_input, &sealed_output),
        Err(ProjectionRelationError::OutputSealedBeforeInput)
    );
    let sealed_lag = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(2),
        StreamOffset::new(2),
        true,
    )
    .emit(range(0, 2), 1)
    .finish()
    .unwrap();
    assert_eq!(
        validate_projection_relation(&input, &sealed_lag),
        Err(ProjectionRelationError::SealedOutputNotCaughtUp)
    );
}

struct LineGate;

impl Projector<char> for LineGate {
    type Output = String;
    type Error = std::convert::Infallible;

    fn project(
        &mut self,
        input: &Projection<char>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        let chars = input
            .spans()
            .iter()
            .flat_map(|span| span.values().iter().copied());
        let mut output = ProjectionBuilder::new(
            input.source_base(),
            input.source_base(),
            input.source_end(),
            false,
        );
        let mut start = input.source_base();
        let mut line = String::new();
        let mut cursor = start;
        for character in chars {
            cursor = cursor.saturating_add(1);
            line.push(character);
            if character == '\n' {
                output = output.emit(
                    range(start.as_u64(), cursor.as_u64()),
                    line.trim_end_matches('\n').to_owned(),
                );
                line.clear();
                start = cursor;
            }
        }
        if start < cursor {
            output = output.emit(range(start.as_u64(), cursor.as_u64()), line);
        }
        let stable = if input.is_sealed() {
            input.source_end()
        } else {
            start
        };
        let mut final_output = ProjectionBuilder::new(
            input.source_base(),
            stable,
            input.source_end(),
            input.is_sealed(),
        );
        for span in output.finish().unwrap().spans() {
            final_output = final_output.emit_many(span.source(), span.values().iter().cloned());
        }
        Ok(final_output.finish().unwrap())
    }
}

#[test]
fn line_gate_projector_converges_incremental_and_batch() {
    let open = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(6),
        StreamOffset::new(9),
        false,
    )
    .emit_many(range(0, 6), "hello\n".chars())
    .emit_many(range(6, 9), "wor".chars())
    .finish()
    .unwrap();
    let sealed = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(12),
        StreamOffset::new(12),
        true,
    )
    .emit_many(range(0, 12), "hello\nworld!".chars())
    .finish()
    .unwrap();
    let mut gate = LineGate;
    let mut batch = LineGate;
    let incremental = gate.project(&sealed).unwrap();
    let one_shot = batch.project(&sealed).unwrap();
    assert_eq!(incremental, one_shot);
    let open_output = gate.project(&open).unwrap();
    assert_eq!(open_output.spans()[0].values(), &["hello"]);
    validate_projection_transition(&open_output, &incremental).unwrap();
}

#[test]
fn transitions_reject_monotonicity_violations_and_sealed_mutations() {
    let previous = ProjectionBuilder::new(
        StreamOffset::new(1),
        StreamOffset::new(2),
        StreamOffset::new(4),
        false,
    )
    .emit(range(1, 2), 1)
    .elide(range(2, 4))
    .finish()
    .unwrap();
    let base_regressed = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(2),
        StreamOffset::new(4),
        false,
    )
    .emit(range(0, 2), 1)
    .elide(range(2, 4))
    .finish()
    .unwrap();
    assert_eq!(
        validate_projection_transition(&previous, &base_regressed),
        Err(ProjectionTransitionError::SourceBaseRegressed)
    );
    let end_regressed = ProjectionBuilder::new(
        StreamOffset::new(1),
        StreamOffset::new(2),
        StreamOffset::new(3),
        false,
    )
    .emit(range(1, 2), 1)
    .elide(range(2, 3))
    .finish()
    .unwrap();
    assert_eq!(
        validate_projection_transition(&previous, &end_regressed),
        Err(ProjectionTransitionError::SourceEndRegressed)
    );
    let stability_regressed = ProjectionBuilder::new(
        StreamOffset::new(1),
        StreamOffset::new(1),
        StreamOffset::new(4),
        false,
    )
    .elide(range(1, 4))
    .finish()
    .unwrap();
    assert_eq!(
        validate_projection_transition(&previous, &stability_regressed),
        Err(ProjectionTransitionError::StabilityRegressed)
    );

    let sealed = complete::<u8>(4, true)
        .emit(range(0, 4), 1)
        .finish()
        .unwrap();
    let unsealed = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(4),
        StreamOffset::new(4),
        false,
    )
    .emit(range(0, 4), 1)
    .finish()
    .unwrap();
    assert_eq!(
        validate_projection_transition(&sealed, &unsealed),
        Err(ProjectionTransitionError::UnsealedAfterSeal)
    );
    let changed = complete::<u8>(4, true)
        .emit(range(0, 4), 2)
        .finish()
        .unwrap();
    assert_eq!(
        validate_projection_transition(&sealed, &changed),
        Err(ProjectionTransitionError::ChangedAfterSeal)
    );
    validate_projection_transition(&sealed, &sealed).unwrap();
}

struct FailFirst;
struct FailSecond;
struct BadRelation;

#[derive(Debug, Clone, Copy)]
struct Failure;
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("failure")
    }
}
impl std::error::Error for Failure {}

impl Projector<u8> for FailFirst {
    type Output = u8;
    type Error = Failure;
    fn project(&mut self, _: &Projection<u8>) -> Result<Projection<u8>, Self::Error> {
        Err(Failure)
    }
}
impl Projector<u8> for FailSecond {
    type Output = u8;
    type Error = Failure;
    fn project(&mut self, _: &Projection<u8>) -> Result<Projection<u8>, Self::Error> {
        Err(Failure)
    }
}
impl Projector<u8> for BadRelation {
    type Output = u8;
    type Error = Failure;
    fn project(&mut self, input: &Projection<u8>) -> Result<Projection<u8>, Self::Error> {
        Ok(ProjectionBuilder::new(
            StreamOffset::new(1),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        )
        .emit(range(1, input.source_end().as_u64()), 1)
        .finish()
        .unwrap())
    }
}

struct Identity;

impl Projector<u8> for Identity {
    type Output = u8;
    type Error = Failure;

    fn project(&mut self, input: &Projection<u8>) -> Result<Projection<u8>, Self::Error> {
        Ok(ProjectionBuilder::new(
            input.source_base(),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        )
        .emit_many(
            range(input.source_base().as_u64(), input.source_end().as_u64()),
            input
                .spans()
                .iter()
                .flat_map(|span| span.values().iter().copied()),
        )
        .finish()
        .unwrap())
    }
}

#[test]
fn composition_reports_each_stage_and_contract() {
    let input = complete::<u8>(4, true)
        .emit(range(0, 4), 1)
        .finish()
        .unwrap();
    let mut first_error = FailFirst.then(Identity);
    assert!(matches!(
        first_error.project(&input),
        Err(ThenError::First(_))
    ));
    let mut first_relation = BadRelation.then(Identity);
    assert!(matches!(
        first_relation.project(&input),
        Err(ThenError::FirstRelation(_))
    ));
    let mut second_error = Identity.then(FailSecond);
    assert!(matches!(
        second_error.project(&input),
        Err(ThenError::Second(_))
    ));
    let mut second_relation = Identity.then(BadRelation);
    assert!(matches!(
        second_relation.project(&input),
        Err(ThenError::SecondRelation(_))
    ));
}

struct TemporalProbe {
    deadline: Option<Instant>,
    advances: Rc<RefCell<Vec<Instant>>>,
    changed: bool,
}

impl Projector<u8> for TemporalProbe {
    type Output = u8;
    type Error = std::convert::Infallible;

    fn project(&mut self, input: &Projection<u8>) -> Result<Projection<u8>, Self::Error> {
        let mut builder = ProjectionBuilder::new(
            input.source_base(),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        );
        for span in input.spans() {
            builder = builder.emit_many(span.source(), span.values().iter().copied());
        }
        Ok(builder.finish().unwrap())
    }

    fn next_wakeup(&self) -> Option<Instant> {
        self.deadline
    }

    fn advance(&mut self, now: Instant) -> bool {
        self.advances.borrow_mut().push(now);
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.deadline = None;
            self.changed = true;
            return true;
        }
        false
    }
}

#[test]
fn temporal_then_uses_the_minimum_deadline_and_shared_now() {
    let now = Instant::now();
    let first_log = Rc::new(RefCell::new(Vec::new()));
    let second_log = Rc::new(RefCell::new(Vec::new()));
    let mut pipeline = TemporalProbe {
        deadline: Some(now + std::time::Duration::from_millis(10)),
        advances: first_log.clone(),
        changed: false,
    }
    .then(TemporalProbe {
        deadline: Some(now + std::time::Duration::from_millis(20)),
        advances: second_log.clone(),
        changed: false,
    });
    assert_eq!(
        pipeline.next_wakeup(),
        Some(now + std::time::Duration::from_millis(10))
    );
    assert!(!pipeline.advance(now));
    assert_eq!(first_log.borrow().as_slice(), &[now]);
    assert_eq!(second_log.borrow().as_slice(), &[now]);
    assert!(pipeline.advance(now + std::time::Duration::from_millis(10)));
    assert_eq!(
        first_log.borrow().as_slice(),
        &[now, now + std::time::Duration::from_millis(10)]
    );
    assert_eq!(
        second_log.borrow().as_slice(),
        &[now, now + std::time::Duration::from_millis(10)]
    );
}

#[test]
fn smooth_composes_temporally_and_project_observes_release() {
    let input = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(3),
        StreamOffset::new(3),
        false,
    )
    .emit(range(0, 1), 1u8)
    .emit(range(1, 2), 2u8)
    .emit(range(2, 3), 3u8)
    .finish()
    .unwrap();
    let config =
        SmoothConfig::try_from_parts(std::time::Duration::from_millis(16), 2.0, 1_000.0, 1_000.0)
            .unwrap();
    let mut pipeline = Smooth::new(config).then(Identity);
    let first = pipeline.project(&input).unwrap();
    assert_eq!(first.source_end(), StreamOffset::new(1));
    let now = Instant::now();
    assert!(!pipeline.advance(now));
    let due = now + std::time::Duration::from_millis(16);
    assert_eq!(pipeline.next_wakeup(), Some(due));
    assert!(pipeline.advance(due));
    let output = pipeline.project(&input).unwrap();
    assert!(output.source_end() > StreamOffset::new(1));
}

#[test]
fn smooth_config_rejects_invalid_temporal_values() {
    assert!(SmoothConfig::try_from_parts(std::time::Duration::ZERO, 1.0, 1.0, 1.0).is_err());
    assert!(
        SmoothConfig::try_from_parts(std::time::Duration::from_millis(1), -1.0, 1.0, 1.0).is_err()
    );
    assert!(
        SmoothConfig::try_from_parts(std::time::Duration::from_millis(1), 1.0, 2.0, 1.0).is_err()
    );
    assert!(
        SmoothConfig::try_from_parts(std::time::Duration::from_millis(1), 0.0, 0.0, 0.0).is_err()
    );
    assert!(
        SmoothConfig::try_from_parts(std::time::Duration::from_millis(1), 0.0, 0.0, 1.0).is_err()
    );
}

struct LocalProjector(Rc<RefCell<u32>>);

impl Projector<u8> for LocalProjector {
    type Output = u8;
    type Error = std::convert::Infallible;

    fn project(&mut self, input: &Projection<u8>) -> Result<Projection<u8>, Self::Error> {
        *self.0.borrow_mut() += 1;
        Ok(ProjectionBuilder::new(
            input.source_base(),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        )
        .emit_many(
            range(input.source_base().as_u64(), input.source_end().as_u64()),
            input
                .spans()
                .iter()
                .flat_map(|span| span.values().iter().copied()),
        )
        .finish()
        .expect("cloned projection remains valid"))
    }
}

#[test]
fn projector_may_be_non_send() {
    let counter = Rc::new(RefCell::new(0));
    let mut projector = LocalProjector(counter.clone());
    let input = complete::<u8>(4, true)
        .emit(range(0, 4), 1)
        .finish()
        .unwrap();
    projector.project(&input).unwrap();
    assert_eq!(*counter.borrow(), 1);
}

#[test]
fn smooth_handles_unstable_tail_replacement_and_span_merges_without_panicking() {
    let mut smooth = Smooth::default();

    // Snapshot 1: a 1-byte span is published immediately.
    let input1 = smooth_input(1, 1, false, &[1]);
    let out1 = smooth.project(&input1).unwrap();
    assert_eq!(out1.source_end(), StreamOffset::new(1));

    // Snapshot 2: the tail span is merged into a longer 3-byte span (e.g. combining character).
    let input2 = smooth_input(3, 3, false, &[3]);
    let out2 = smooth.project(&input2).unwrap();
    // out2 does not panic and yields up to the last whole span that fits.
    assert!(out2.source_end() <= StreamOffset::new(3));

    // After advancing, the new merged span is released.
    let now = std::time::Instant::now() + std::time::Duration::from_millis(50);
    smooth.advance(now);
    let out3 = smooth.project(&input2).unwrap();
    assert_eq!(out3.source_end(), StreamOffset::new(3));
}
