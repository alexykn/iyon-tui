use pulldown_cmark::{Alignment, Event, Options, Parser, Tag, TagEnd};

fn events(source: &str, options: Options) -> Vec<(Event<'_>, std::ops::Range<usize>)> {
    Parser::new_ext(source, options)
        .into_offset_iter()
        .collect()
}

fn text_event<'a>(
    events: &'a [(Event<'a>, std::ops::Range<usize>)],
) -> Option<(&'a str, std::ops::Range<usize>)> {
    events.iter().find_map(|(event, range)| match event {
        Event::Text(text) => Some((text.as_ref(), range.clone())),
        _ => None,
    })
}

#[test]
fn representative_event_ranges_are_bounded_and_utf8_aligned() {
    let cases = [
        ("plain", "plain", Options::empty()),
        ("escaped", r#"\\*literal* &amp;"#, Options::empty()),
        (
            "emphasis",
            "*em* **strong** ~~strike~~",
            Options::ENABLE_STRIKETHROUGH,
        ),
        ("inline code", "`code`", Options::empty()),
        (
            "links",
            "[inline](url) [ref][id]\n\n[id]: /target\n",
            Options::empty(),
        ),
        ("atx", "# heading\n", Options::empty()),
        ("setext", "heading\n=======\n", Options::empty()),
        ("fence", "```rust\nfn main() {}\n```\n", Options::empty()),
        ("indented", "    code\n", Options::empty()),
        ("quote", "> quote\n", Options::empty()),
        ("tight list", "- a\n- b\n", Options::empty()),
        ("loose list", "- a\n\n- b\n", Options::empty()),
        ("ordered", "1) a\n2) b\n", Options::empty()),
        ("task", "- [x] done\n", Options::ENABLE_TASKLISTS),
        (
            "html",
            "<span>inline</span>\n\n<div>block</div>\n",
            Options::empty(),
        ),
        (
            "table",
            "| a | b |\n| :- | -: |\n| c | d |\n",
            Options::ENABLE_TABLES,
        ),
        ("breaks", "soft\nbreak\n\nhard  \nbreak\n", Options::empty()),
        ("rule", "---\n", Options::empty()),
    ];

    for (name, source, options) in cases {
        for (_, range) in events(source, options) {
            assert!(range.start <= range.end, "{name}: invalid range {range:?}");
            assert!(range.end <= source.len(), "{name}: out of bounds {range:?}");
            assert!(source.is_char_boundary(range.start));
            assert!(source.is_char_boundary(range.end));
        }
    }
}

#[test]
fn inline_code_and_fenced_code_ranges_are_characterized() {
    let inline = events("`code`", Options::empty());
    assert!(inline.iter().any(|(event, range)| {
        matches!(event, Event::Code(text) if text.as_ref() == "code" && range.clone() == (0..6))
    }));

    let source = "```json\nabc\n```\n";
    let fenced = events(source, Options::empty());
    assert!(fenced.iter().any(|(event, range)| {
        matches!(event, Event::Start(Tag::CodeBlock(_)) if range.clone() == (0..15))
    }));
    assert!(fenced.iter().any(|(event, range)| {
        matches!(event, Event::Text(text) if text.as_ref() == "abc\n" && range.clone() == (8..12))
    }));
    assert!(fenced.iter().any(|(event, range)| {
        matches!(event, Event::End(TagEnd::CodeBlock) if range.clone() == (0..15))
    }));
}

#[test]
fn tight_loose_lists_and_ordered_delimiters_are_characterized() {
    let tight = events("- a\n- b\n", Options::empty());
    assert!(
        !tight
            .iter()
            .any(|(event, _)| matches!(event, Event::Start(Tag::Paragraph)))
    );
    let loose = events("- a\n\n- b\n", Options::empty());
    assert_eq!(
        loose
            .iter()
            .filter(|(event, _)| matches!(event, Event::Start(Tag::Paragraph)))
            .count(),
        2
    );
    let ordered = events("1) a\n2) b\n", Options::empty());
    assert!(ordered.iter().any(|(event, range)| {
        matches!(event, Event::Start(Tag::List(Some(1))) if range.clone() == (0..10))
    }));
    assert_eq!(&"1) a\n2) b\n"[0..2], "1)");
}

#[test]
fn references_tables_breaks_and_transformations_are_characterized() {
    let reference = events("[foo][id]\n\n[id]: /x\n", Options::empty());
    assert!(reference.iter().any(|(event, range)| {
        matches!(event, Event::Start(Tag::Link { dest_url, .. }) if dest_url.as_ref() == "/x" && range.clone() == (0..9))
    }));
    assert!(reference.iter().any(|(event, range)| {
        matches!(event, Event::Text(text) if text.as_ref() == "foo" && range.clone() == (1..4))
    }));

    let table = events(
        "| a | b |\n| :- | -: |\n| c | d |\n",
        Options::ENABLE_TABLES,
    );
    assert!(table.iter().any(|(event, _)| {
        matches!(event, Event::Start(Tag::Table(columns)) if columns.as_slice() == [Alignment::Left, Alignment::Right])
    }));
    assert!(
        table
            .iter()
            .any(|(event, _)| matches!(event, Event::Start(Tag::TableHead)))
    );
    assert!(
        table
            .iter()
            .any(|(event, _)| matches!(event, Event::Start(Tag::TableRow)))
    );
    assert!(
        table
            .iter()
            .any(|(event, _)| matches!(event, Event::Start(Tag::TableCell)))
    );

    let breaks = events("soft\nbreak\n\nhard  \nbreak\n", Options::empty());
    assert!(
        breaks
            .iter()
            .any(|(event, range)| matches!(event, Event::SoftBreak) && range.clone() == (4..5))
    );
    assert!(
        breaks
            .iter()
            .any(|(event, range)| matches!(event, Event::HardBreak) && range.clone() == (16..19))
    );

    let transformed = events(r#"\\*literal* &amp;"#, Options::empty());
    let (text, range) = text_event(&transformed).unwrap();
    assert!(text != &r#"\\*literal* &amp;"#[..]);
    assert!(range.start < range.end);
}

#[test]
fn task_html_and_rules_have_expected_event_shapes() {
    let task = events("- [x] done\n", Options::ENABLE_TASKLISTS);
    assert!(
        task.iter()
            .any(|(event, _)| matches!(event, Event::TaskListMarker(true)))
    );
    let html = events(
        "<span>inline</span>\n\n<div>block</div>\n",
        Options::empty(),
    );
    assert!(
        html.iter()
            .any(|(event, _)| matches!(event, Event::InlineHtml(_)))
    );
    assert!(
        html.iter()
            .any(|(event, _)| matches!(event, Event::Html(_)))
    );
    let rule = events("---\n", Options::empty());
    assert!(
        rule.iter()
            .any(|(event, range)| matches!(event, Event::Rule) && range.clone() == (0..4))
    );
}
