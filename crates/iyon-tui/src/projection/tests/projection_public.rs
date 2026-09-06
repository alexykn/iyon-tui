use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use iyon_tui::projection::{ProjectionBuilder, validate_projection_relation};
use iyon_tui::stream::{StreamOffset, StreamRange};
use iyon_tui::{Projection, Projector, ProjectorExt, Smooth, SmoothConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Record(&'static str);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fragment(String);

#[derive(Debug)]
struct ToFragments {
    calls: Rc<RefCell<u32>>,
}

impl Projector<Record> for ToFragments {
    type Output = Fragment;
    type Error = std::convert::Infallible;

    fn project(
        &mut self,
        input: &Projection<Record>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        *self.calls.borrow_mut() += 1;
        let mut builder = ProjectionBuilder::new(
            input.source_base(),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        );
        for span in input.spans() {
            builder = builder.emit_many(
                span.source(),
                span.values()
                    .iter()
                    .map(|record| Fragment(record.0.to_owned())),
            );
        }
        Ok(builder.finish().unwrap())
    }
}

struct ToLines;

impl Projector<Fragment> for ToLines {
    type Output = String;
    type Error = std::convert::Infallible;

    fn project(
        &mut self,
        input: &Projection<Fragment>,
    ) -> Result<Projection<Self::Output>, Self::Error> {
        let mut builder = ProjectionBuilder::new(
            input.source_base(),
            input.stable_through(),
            input.source_end(),
            input.is_sealed(),
        );
        for span in input.spans() {
            builder = builder.emit_many(
                span.source(),
                span.values().iter().map(|fragment| fragment.0.clone()),
            );
        }
        Ok(builder.finish().unwrap())
    }

    fn restart_from(&self, output_from: StreamOffset) -> StreamOffset {
        output_from
            .as_u64()
            .checked_sub(1)
            .map_or(StreamOffset::ZERO, StreamOffset::new)
    }
}

fn source() -> Projection<Record> {
    ProjectionBuilder::new(
        StreamOffset::new(10),
        StreamOffset::new(12),
        StreamOffset::new(12),
        true,
    )
    .emit(
        StreamRange::new(StreamOffset::new(10), StreamOffset::new(11)),
        Record("log"),
    )
    .elide(StreamRange::new(
        StreamOffset::new(11),
        StreamOffset::new(12),
    ))
    .finish()
    .unwrap()
}

#[test]
fn external_consumer_can_build_and_compose_projections() {
    let calls = Rc::new(RefCell::new(0));
    let projector = ToFragments {
        calls: calls.clone(),
    }
    .then(ToLines);
    let mut projector = projector;
    let output = projector.project(&source()).unwrap();

    assert_eq!(*calls.borrow(), 1);
    assert_eq!(output.source_base(), StreamOffset::new(10));
    assert_eq!(output.source_end(), StreamOffset::new(12));
    assert_eq!(output.spans()[0].values(), &["log".to_owned()]);
    assert!(output.spans()[1].values().is_empty());
    validate_projection_relation(&source(), &output).unwrap();
}

#[test]
fn non_send_projector_is_usable_and_restart_backchains() {
    let projector = ToFragments {
        calls: Rc::new(RefCell::new(0)),
    }
    .then(ToLines);
    assert_eq!(
        projector.restart_from(StreamOffset::new(10)),
        StreamOffset::new(9)
    );
}

#[test]
fn fields_are_exposed_only_through_accessors() {
    let projection = source();
    assert_eq!(projection.spans().len(), 2);
    assert_eq!(projection.spans()[0].source().len(), 1);
}

#[test]
fn smooth_has_deterministic_deadlines_and_sealed_identity() {
    let input = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(3),
        StreamOffset::new(3),
        false,
    )
    .emit(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(1)),
        1u8,
    )
    .emit(
        StreamRange::new(StreamOffset::new(1), StreamOffset::new(2)),
        2u8,
    )
    .emit(
        StreamRange::new(StreamOffset::new(2), StreamOffset::new(3)),
        3u8,
    )
    .finish()
    .unwrap();
    let config = SmoothConfig::try_from_parts(Duration::from_millis(10), 1.0, 20.0, 20.0).unwrap();
    let mut smooth = Smooth::new(config);
    let first = smooth.project(&input).unwrap();
    assert_eq!(first.source_end(), StreamOffset::new(1));
    assert_eq!(smooth.next_wakeup(), None);

    let t0 = Instant::now();
    assert!(!smooth.advance(t0));
    assert_eq!(smooth.next_wakeup(), Some(t0 + Duration::from_millis(10)));
    assert!(!smooth.advance(t0 + Duration::from_millis(9)));
    let _ = smooth.advance(t0 + Duration::from_millis(10));

    let sealed = ProjectionBuilder::new(
        StreamOffset::ZERO,
        StreamOffset::new(3),
        StreamOffset::new(3),
        true,
    )
    .emit(
        StreamRange::new(StreamOffset::ZERO, StreamOffset::new(1)),
        1u8,
    )
    .emit(
        StreamRange::new(StreamOffset::new(1), StreamOffset::new(2)),
        2u8,
    )
    .emit(
        StreamRange::new(StreamOffset::new(2), StreamOffset::new(3)),
        3u8,
    )
    .finish()
    .unwrap();
    assert_eq!(smooth.project(&sealed).unwrap(), sealed);
    assert_eq!(smooth.next_wakeup(), None);
}
