use iyon_tui::projection::{ProjectionBuilder, validate_projection_relation};
use iyon_tui::stream::{StreamOffset, StreamRange};
use iyon_tui::text::{
    BlockKind, CodeBlock, HeadingLevel, InlineKind, LanguageId, List, ListItem, Mark, TextRun,
};
use iyon_tui::{Block, HistoryLayout, Inline, InlineContent, Insets, SmoothConfig};

#[test]
fn ordinary_text_construction_matches_explicit_ir() {
    let convenience = Block::paragraph("hello");
    let explicit = Block::paragraph(InlineContent::new([Inline::text(TextRun::synthetic(
        "hello",
    ))]));
    assert_eq!(convenience, explicit);

    assert_eq!(HeadingLevel::H1.get(), 1);
    assert_eq!(Inline::text("x").strong().marks().marks(), &[Mark::Strong]);
    assert_eq!(List::bulleted([ListItem::paragraph("item")]).tight(), true);

    let language = LanguageId::new("json").unwrap();
    let code = CodeBlock::new(Some(language), None::<&str>, "{}");
    assert!(matches!(Block::code(code).kind(), BlockKind::CodeBlock(_)));
    assert!(matches!(Inline::text("x").kind(), InlineKind::Text(_)));
}

#[test]
fn projection_helpers_preserve_envelope_and_relation() {
    let input = ProjectionBuilder::new(
        StreamOffset::new(10),
        StreamOffset::new(12),
        StreamOffset::new(12),
        true,
    )
    .emit(
        StreamRange::new(StreamOffset::new(10), StreamOffset::new(12)),
        7u8,
    )
    .finish()
    .unwrap();
    let mapped = input.map_ref(|value| value.to_string());
    assert_eq!(mapped.source_base(), input.source_base());
    assert_eq!(mapped.spans()[0].values(), &["7"]);
    validate_projection_relation(&input, &mapped).unwrap();

    let split = input.map_spans(|span| vec![*span.values().first().unwrap(), 8]);
    assert_eq!(split.spans().len(), 1);
    assert_eq!(split.spans()[0].values(), &[7, 8]);
}

#[test]
fn smooth_and_history_configuration_are_default_first() {
    assert_eq!(SmoothConfig::new(), SmoothConfig::default());
    assert_eq!(
        HistoryLayout::new()
            .with_padding(Insets::all(1))
            .with_gap(2),
        HistoryLayout::from_parts(Insets::all(1), 2)
    );
}
