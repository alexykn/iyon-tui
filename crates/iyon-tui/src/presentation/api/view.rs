//! Semantic View construction and the open conversion boundary.

use std::sync::Arc;

use super::{
    composition::{Horizontal, Vertical},
    grid::Grid,
    style::{
        BorderSpec, ColorSpec, Insets, OverflowIndicator, StyleFacts, StyleRef, StyleSpec,
        StyleStateKey, StyleStateValue, StyleStates, TextAttribute, VerticalAlign,
    },
    text::{HorizontalAlign, Text, TextSpan, WrapMode},
};
use crate::presentation::ir::{
    ClampRowsView, ColumnChild, ColumnView, ContainerNode, Decoration, HangingView, HeightRule,
    PersistentSeq, RowChild, RowView, View, ViewKind, ViewNodeParts, WidthRule,
};

/// Validated common-property candidate assembled by native ingress and
/// applied in one final root by [`View::native_patched`]. Every field is
/// optional: `None` keeps the base value. `width_fill`/`height_fill` map
/// `Some(true)` to `Fill` and `Some(false)` to `Fit`; size-rule vocabulary
/// itself stays inside the core.
#[cfg(feature = "native-host")]
#[derive(Clone, Debug, Default)]
#[doc(hidden)]
pub struct NativeCommonPatch {
    pub padding: Option<Insets>,
    pub background: Option<ColorSpec>,
    pub foreground: Option<ColorSpec>,
    pub border: Option<BorderSpec>,
    pub style: Option<StyleRef>,
    pub style_states: Vec<(StyleStateKey, StyleStateValue)>,
    pub width_fill: Option<bool>,
    pub height_fill: Option<bool>,
    pub min_width: Option<u16>,
    pub max_width: Option<u16>,
    pub min_height: Option<u16>,
    pub max_height: Option<u16>,
}

impl View {
    pub(crate) fn new_kind(kind: ViewKind) -> Self {
        Self::from_node(ViewNodeParts {
            width: WidthRule::Fit,
            height: HeightRule::Fit,
            decoration: Decoration::default(),
            style_states: StyleStates::default(),
            style_facts: StyleFacts::default(),
            state_attachment: None,
            content_attachment: None,
            kind,
        })
    }

    fn wrap_structural(self, make_kind: impl FnOnce(View) -> ViewKind) -> Self {
        let width = self.width();
        let height = self.height();
        Self::from_node(ViewNodeParts {
            width,
            height,
            decoration: Decoration::default(),
            style_states: StyleStates::default(),
            style_facts: StyleFacts::default(),
            state_attachment: None,
            content_attachment: None,
            kind: make_kind(self),
        })
    }

    pub fn text(text: impl Into<String>) -> Text {
        Text::plain(text)
    }

    pub fn styled_text(spans: impl IntoIterator<Item = TextSpan>) -> Text {
        Text::styled(spans)
    }

    /// Constructs horizontal composition immediately with a `Fit` width.
    /// The builder defaults to zero gap and top vertical alignment.
    pub fn horizontal(build: impl FnOnce(&mut Horizontal)) -> Self {
        let mut horizontal = Horizontal::new();
        build(&mut horizontal);
        let (children, gap, vertical_align) = horizontal.into_parts();

        Self::new_kind(ViewKind::Row(Arc::new(RowView {
            children: PersistentSeq::from_vec(children),
            gap,
            vertical_align,
        })))
    }

    /// Constructs vertical composition immediately with a `Fit` width and
    /// zero gap.
    pub fn vertical(build: impl FnOnce(&mut Vertical)) -> Self {
        let mut vertical = Vertical::new();
        build(&mut vertical);
        let (children, gap) = vertical.into_parts();

        Self::new_kind(ViewKind::Column(Arc::new(ColumnView {
            children: PersistentSeq::from_vec(children),
            gap,
        })))
    }

    /// Direct row construction for built-in producers: moves already-built
    /// children into content tracks with one root, matching the
    /// [`Horizontal`](super::composition::Horizontal) defaults (zero gap is
    /// the caller's choice; vertical alignment is top).
    pub(crate) fn row_from_views(children: Vec<View>, gap: u16) -> Self {
        Self::new_kind(ViewKind::Row(Arc::new(RowView {
            children: PersistentSeq::from_vec(
                children.into_iter().map(RowChild::content).collect(),
            ),
            gap,
            vertical_align: VerticalAlign::Top,
        })))
    }

    /// Direct column construction for built-in producers: moves already-built
    /// children into content tracks with one root, matching the
    /// [`Vertical`](super::composition::Vertical) defaults.
    pub(crate) fn column_from_views(children: Vec<View>, gap: u16) -> Self {
        Self::new_kind(ViewKind::Column(Arc::new(ColumnView {
            children: PersistentSeq::from_vec(
                children.into_iter().map(ColumnChild::content).collect(),
            ),
            gap,
        })))
    }

    /// Constructs two-dimensional composition immediately with `Fit` width and
    /// height. Track allocation is shared across rows; spanning cells occupy
    /// the full area of their tracks, including internal gaps.
    pub fn grid(build: impl FnOnce(&mut Grid)) -> Self {
        let mut grid = Grid::new();
        build(&mut grid);
        Self::new_kind(ViewKind::Grid(Arc::new(grid.into_grid_view())))
    }

    /// Constructs a semantic hanging row.
    ///
    /// Constructs `prefix + hanging body + repeated continuation indentation`.
    ///
    /// The `prefix` is rendered once on the first body row. The
    /// `continuation_prefix` is repeated beside subsequent body rows. The
    /// `body` is an ordinary semantic view, resolved and mounted once, and
    /// may contain Components. Hanging lays the body out once; repetition is
    /// presentation of the continuation prefix rather than repeated content.
    ///
    /// The continuation prefix cannot contain component identity because it
    /// is physically repeated under the current one-placement component
    /// model. Prefix and body Components are supported normally.
    ///
    /// If the prefix leaves no body capacity, the compiled view is marked
    /// physically incomplete rather than silently dropping or relocating body
    /// content. Rendering remains panic-free; terminal-size rejection is a
    /// separate policy.
    pub fn hanging(
        prefix: impl IntoView,
        continuation_prefix: impl IntoView,
        body: impl IntoView,
    ) -> Self {
        let prefix = prefix.into_view();
        let continuation_prefix = continuation_prefix.into_view();
        let body = body.into_view();
        assert!(
            !continuation_prefix.contains_component_identity(),
            "hanging continuation_prefix cannot contain component identity: it is repeated"
        );

        Self::new_kind(ViewKind::Hanging(Arc::new(HangingView {
            prefix,
            continuation_prefix,
            body,
        })))
    }

    pub(crate) fn column(children: Vec<View>, gap: u16) -> Self {
        Self::new_kind(ViewKind::Column(Arc::new(ColumnView {
            children: PersistentSeq::from_vec(
                children
                    .into_iter()
                    .map(ColumnChild::content)
                    .collect::<Vec<_>>(),
            ),
            gap,
        })))
    }

    /// Creates a new undecorated structural boundary around this view.
    #[must_use]
    pub fn container(self) -> Self {
        self.wrap_structural(|child| ViewKind::Container(Arc::new(ContainerNode { child })))
    }

    /// Creates a private vertical viewport around a semantic view.
    pub(crate) fn row_viewport(child: View, skip_rows: u16) -> Self {
        Self::from_node(ViewNodeParts {
            width: WidthRule::Fill,
            height: HeightRule::Fill,
            decoration: Decoration::default(),
            style_states: StyleStates::default(),
            style_facts: StyleFacts::default(),
            state_attachment: None,
            content_attachment: None,
            kind: ViewKind::RowViewport(Arc::new(crate::presentation::ir::RowViewportView {
                child,
                skip_rows,
                visible_height: None,
                layout_height: None,
                intrinsic_content_height: true,
            })),
        })
    }

    /// Creates a private vertical viewport with an explicit visible height.
    /// The child is still laid out at its full intrinsic height so component
    /// allocation remains truthful while painting and visibility are clipped.
    pub(crate) fn row_viewport_with_height(
        child: View,
        skip_rows: u16,
        visible_height: Option<u16>,
    ) -> Self {
        Self::from_node(ViewNodeParts {
            width: WidthRule::Fill,
            height: HeightRule::Fill,
            decoration: Decoration::default(),
            style_states: StyleStates::default(),
            style_facts: StyleFacts::default(),
            state_attachment: None,
            content_attachment: None,
            kind: ViewKind::RowViewport(Arc::new(crate::presentation::ir::RowViewportView {
                child,
                skip_rows,
                visible_height,
                layout_height: None,
                intrinsic_content_height: false,
            })),
        })
    }

    pub(crate) fn bounded_row_viewport(child: View, height: u16) -> Self {
        Self::from_node(ViewNodeParts {
            width: WidthRule::Fill,
            height: HeightRule::Fill,
            decoration: Decoration::default(),
            style_states: StyleStates::default(),
            style_facts: StyleFacts::default(),
            state_attachment: None,
            content_attachment: None,
            kind: ViewKind::RowViewport(Arc::new(crate::presentation::ir::RowViewportView {
                child,
                skip_rows: 0,
                visible_height: Some(height),
                layout_height: Some(height),
                intrinsic_content_height: false,
            })),
        })
    }

    /// Creates a new structural truncation boundary around this view.
    #[must_use]
    pub fn clamp_rows(self, max_rows: u16, overflow: OverflowIndicator) -> Self {
        self.wrap_structural(|child| {
            ViewKind::ClampRows(Arc::new(ClampRowsView {
                child,
                max_rows,
                overflow,
            }))
        })
    }

    #[must_use]
    pub fn spacer(rows: u16) -> Self {
        Self::new_kind(ViewKind::Spacer { rows })
    }

    /// Creates the native side of a retained ContentHost attachment. The
    /// public TypeScript semantic node carries a backend-neutral HandleId;
    /// structural decoding resolves it to this host-local identity. The
    /// attachment is part of the single final root: no intermediate node is
    /// allocated just to attach the port identity.
    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_content_host(port_id: u64) -> Result<Self, String> {
        if port_id == 0 {
            return Err("ContentPort identity must be positive".to_owned());
        }
        Ok(Self::from_node(ViewNodeParts {
            width: WidthRule::Fit,
            height: HeightRule::Fit,
            decoration: Decoration::default(),
            style_states: StyleStates::default(),
            style_facts: StyleFacts::default(),
            state_attachment: None,
            content_attachment: Some(port_id),
            kind: ViewKind::ContentHost,
        }))
    }

    /// Assigns one application-owned semantic styling dimension to this View
    /// subtree. Appearance is resolved later during painting.
    #[must_use]
    pub fn style_state(
        self,
        key: impl Into<StyleStateKey>,
        value: impl Into<StyleStateValue>,
    ) -> Self {
        self.map_node(|node| node.style_states.set(key, value))
    }

    /// Assigns a self-only semantic styling fact.
    #[allow(dead_code)]
    pub(crate) fn style_fact(
        self,
        key: impl Into<StyleStateKey>,
        value: impl Into<StyleStateValue>,
    ) -> Self {
        self.map_node(|node| node.style_facts.set(key, value))
    }

    pub(crate) fn with_style_facts(self, facts: StyleFacts) -> Self {
        self.map_node(|node| node.style_facts = facts)
    }

    /// Assigns multiple application-owned semantic styling dimensions.
    #[must_use]
    pub fn style_states(
        self,
        states: impl IntoIterator<Item = (StyleStateKey, StyleStateValue)>,
    ) -> Self {
        self.map_node(|node| {
            for (key, value) in states {
                node.style_states.set(key, value);
            }
        })
    }

    /// Sets the current node's padding; repeated calls replace the prior value.
    #[must_use]
    pub fn padding(self, padding: impl Into<Insets>) -> Self {
        self.map_node(|node| node.decoration.padding = padding.into())
    }

    /// Paints the current node's allocated surface, including transparent tails.
    #[must_use]
    pub fn background(self, color: ColorSpec) -> Self {
        self.map_node(|node| node.decoration.surface_background = Some(color))
    }

    /// Applies a semantic named style or direct sparse style to this node.
    #[must_use]
    pub fn style(self, style: impl Into<StyleRef>) -> Self {
        let style = style.into();
        self.map_node(|node| {
            if style.theme.is_some() {
                node.decoration.text_style = style;
            } else {
                node.decoration.text_style.overlay(&style.local);
            }
        })
    }

    /// Sets inherited foreground intent for descendant text.
    #[must_use]
    pub fn foreground(self, color: ColorSpec) -> Self {
        self.map_node(|node| {
            node.decoration
                .text_style
                .overlay(&StyleSpec::new().foreground(color));
        })
    }

    /// Replaces the current node's complete border specification.
    #[must_use]
    pub fn border(self, border: BorderSpec) -> Self {
        self.map_node(|node| node.decoration.border = Some(border))
    }

    /// Sets sparse inherited text-attribute intent, including explicit false.
    #[must_use]
    pub fn text_attribute(self, attribute: TextAttribute, enabled: bool) -> Self {
        self.map_node(|node| {
            node.decoration
                .text_style
                .overlay(&StyleSpec::new().attribute(attribute, enabled));
        })
    }

    #[must_use]
    pub fn bold(self) -> Self {
        self.text_attribute(TextAttribute::Bold, true)
    }

    #[must_use]
    pub fn dim(self) -> Self {
        self.text_attribute(TextAttribute::Dim, true)
    }

    #[must_use]
    pub fn italic(self) -> Self {
        self.text_attribute(TextAttribute::Italic, true)
    }

    #[must_use]
    pub fn underline(self) -> Self {
        self.text_attribute(TextAttribute::Underline, true)
    }

    #[must_use]
    pub fn reversed(self) -> Self {
        self.text_attribute(TextAttribute::Reversed, true)
    }

    #[must_use]
    pub fn strikethrough(self) -> Self {
        self.text_attribute(TextAttribute::Strikethrough, true)
    }

    #[must_use]
    pub fn fit_width(self) -> Self {
        self.map_node(|node| node.width = WidthRule::Fit)
    }

    #[must_use]
    pub fn fill_width(self) -> Self {
        self.map_node(|node| node.width = WidthRule::Fill)
    }

    #[must_use]
    pub fn fit_height(self) -> Self {
        self.map_node(|node| node.height = HeightRule::Fit)
    }

    #[must_use]
    pub fn fill_height(self) -> Self {
        self.map_node(|node| node.height = HeightRule::Fill)
    }

    /// Sets the minimum outer width this View may receive.
    #[must_use]
    pub fn min_width(self, width: u16) -> Self {
        self.map_node(|node| node.decoration.bounds.width.min = width)
    }

    /// Sets the maximum outer width this View may receive.
    #[must_use]
    pub fn max_width(self, width: u16) -> Self {
        self.map_node(|node| node.decoration.bounds.width.max = width)
    }

    /// Sets the minimum outer height this View may receive.
    #[must_use]
    pub fn min_height(self, height: u16) -> Self {
        self.map_node(|node| node.decoration.bounds.height.min = height)
    }

    /// Sets the maximum outer height this View may receive.
    #[must_use]
    pub fn max_height(self, height: u16) -> Self {
        self.map_node(|node| node.decoration.bounds.height.max = height)
    }

    /// Assembles one final root from a base plus common overrides, applying
    /// the §6.3 precedence order. Native ingress decodes into this candidate
    /// instead of chaining one-root-per-field modifiers.
    #[cfg(feature = "native-host")]
    #[doc(hidden)]
    pub fn native_patched(base: View, patch: &NativeCommonPatch) -> Self {
        let mut parts = ViewNodeParts::from_view(&base);
        if let Some(padding) = patch.padding {
            parts.decoration.padding = padding;
        }
        if let Some(background) = &patch.background {
            parts.decoration.surface_background = Some(background.clone());
        }
        if let Some(foreground) = &patch.foreground {
            parts
                .decoration
                .text_style
                .overlay(&StyleSpec::new().foreground(foreground.clone()));
        }
        if let Some(border) = &patch.border {
            parts.decoration.border = Some(border.clone());
        }
        if let Some(style) = &patch.style {
            if style.theme.is_some() {
                parts.decoration.text_style = style.clone();
            } else {
                parts.decoration.text_style.overlay(&style.local);
            }
        }
        for (key, value) in &patch.style_states {
            parts.style_states.set(key.clone(), value.clone());
        }
        if let Some(fill) = patch.width_fill {
            parts.width = if fill {
                WidthRule::Fill
            } else {
                WidthRule::Fit
            };
        }
        if let Some(fill) = patch.height_fill {
            parts.height = if fill {
                HeightRule::Fill
            } else {
                HeightRule::Fit
            };
        }
        if let Some(min) = patch.min_width {
            parts.decoration.bounds.width.min = min;
        }
        if let Some(max) = patch.max_width {
            parts.decoration.bounds.width.max = max;
        }
        if let Some(min) = patch.min_height {
            parts.decoration.bounds.height.min = min;
        }
        if let Some(max) = patch.max_height {
            parts.decoration.bounds.height.max = max;
        }
        Self::from_node(parts)
    }

    /// Applies text layout metadata while retaining the existing text payload.
    /// The canonical retained lowering uses this to keep span storage shared.
    #[doc(hidden)]
    #[must_use]
    pub fn with_text_layout(self, wrap: WrapMode, align: HorizontalAlign) -> Self {
        self.with_text_layout_patch(Some(wrap), Some(align))
    }

    /// Applies only the supplied text layout fields, preserving all others.
    #[doc(hidden)]
    #[must_use]
    pub fn with_text_layout_patch(
        self,
        wrap: Option<WrapMode>,
        align: Option<HorizontalAlign>,
    ) -> Self {
        self.map_text(|text| {
            if let Some(wrap) = wrap {
                text.wrap = wrap;
            }
            if let Some(align) = align {
                text.align = align;
            }
        })
    }

    /// Applies a retained text-layout patch without panicking on a non-text
    /// base. Generated native ABI calls use this checked boundary.
    #[doc(hidden)]
    pub fn try_with_text_layout_patch(
        self,
        wrap: Option<WrapMode>,
        align: Option<HorizontalAlign>,
    ) -> Result<Self, &'static str> {
        if !matches!(self.kind(), ViewKind::Text(_)) {
            return Err("text layout patch base is not text");
        }
        Ok(self.with_text_layout_patch(wrap, align))
    }
}

/// Explicit conversion from semantic construction values into the canonical
/// owned [`View`] representation.
pub trait IntoView {
    fn into_view(self) -> View;
}

impl IntoView for View {
    fn into_view(self) -> View {
        self
    }
}

impl IntoView for Text {
    fn into_view(self) -> View {
        self.into_canonical_view()
    }
}

impl IntoView for String {
    fn into_view(self) -> View {
        View::text(self).into_view()
    }
}

impl<'a> IntoView for &'a str {
    fn into_view(self) -> View {
        View::text(self).into_view()
    }
}

impl<'a> IntoView for &'a String {
    fn into_view(self) -> View {
        View::text(self.as_str()).into_view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::api::style::{
        BorderSpec, ColorSpec, Insets, OverflowIndicator, StyleSpec, TextAttribute,
    };
    use crate::presentation::api::text::{HorizontalAlign, Text, WrapMode};

    #[test]
    fn into_view_conversions_are_owned_and_view_conversion_is_identity() {
        let original = View::column(vec![View::spacer(1)], 0);
        assert_eq!(original.clone().into_view(), original);

        let string_view = String::from("hello").into_view();
        let borrowed_source = String::from("hello");
        let borrowed_view = (&borrowed_source).into_view();
        let str_view = "hello".into_view();
        let expected = View::text("hello").into_view();
        assert_eq!(string_view, expected);
        assert_eq!(borrowed_view, expected);
        assert_eq!(str_view, expected);

        let mut source = String::from("owned");
        let view = (&source).into_view();
        source.clear();
        source.push_str("changed");
        let ViewKind::Text(text) = view.kind() else {
            panic!("expected text view");
        };
        assert_eq!(text.spans[0].text(), "owned");
    }

    #[derive(Debug)]
    struct CustomStatus {
        value: String,
    }

    impl IntoView for CustomStatus {
        fn into_view(self) -> View {
            View::text(self.value)
                .style(StyleSpec::new().bold())
                .into_view()
        }
    }

    #[test]
    fn custom_into_view_implementation_uses_open_boundary() {
        let view = CustomStatus {
            value: "status".to_string(),
        }
        .into_view();
        assert_eq!(view.decoration().text_style.attributes.bold, Some(true));
        assert!(matches!(view.kind(), ViewKind::Text(_)));
    }

    #[test]
    fn semantic_composition_is_owned_data() {
        let view = View::horizontal(|row| {
            row.child(View::text("●").no_wrap());
            row.flex(View::text("long command").fill_width());
            row.gap(1);
        })
        .background(ColorSpec::Theme("accent".into()))
        .padding(Insets::all(1));
        assert!(matches!(view.kind(), ViewKind::Row(_)));
        assert!(!view.decoration().padding.eq(&Insets::ZERO));
    }

    #[test]
    fn view_properties_mutate_the_current_node_and_are_last_write_wins() {
        let view = View::vertical(|_| {})
            .fit_width()
            .padding(1)
            .padding(Insets::vertical(2))
            .background(ColorSpec::ansi(1))
            .background(ColorSpec::ansi(2))
            .foreground(ColorSpec::ansi(3))
            .foreground(ColorSpec::ansi(4))
            .border(BorderSpec::plain())
            .border(BorderSpec::rounded().color(ColorSpec::ansi(5)))
            .bold()
            .dim()
            .italic()
            .underline()
            .reversed()
            .text_attribute(TextAttribute::Bold, false);

        assert!(matches!(view.kind(), ViewKind::Column(_)));
        assert_eq!(view.width(), WidthRule::Fit);
        assert_eq!(view.decoration().padding, Insets::vertical(2));
        assert_eq!(
            view.decoration().surface_background,
            Some(ColorSpec::ansi(2))
        );
        assert_eq!(
            view.decoration().text_style.foreground,
            Some(ColorSpec::ansi(4))
        );
        assert_eq!(view.decoration().text_style.attributes.bold, Some(false));
        assert_eq!(view.decoration().text_style.attributes.dim, Some(true));
        assert_eq!(view.decoration().text_style.attributes.italic, Some(true));
        assert_eq!(
            view.decoration().text_style.attributes.underline,
            Some(true)
        );
        assert_eq!(view.decoration().text_style.attributes.reversed, Some(true));
        assert_eq!(
            view.decoration().border,
            Some(BorderSpec::rounded().color(ColorSpec::ansi(5)))
        );
    }

    #[test]
    fn independent_properties_are_structurally_commutative() {
        let first = View::vertical(|_| {})
            .padding(Insets::horizontal(1))
            .background(ColorSpec::ansi(1))
            .foreground(ColorSpec::ansi(2))
            .bold();
        let second = View::vertical(|_| {})
            .bold()
            .foreground(ColorSpec::ansi(2))
            .background(ColorSpec::ansi(1))
            .padding(Insets::horizontal(1));

        assert_eq!(first, second);
    }

    #[test]
    fn container_creates_an_outer_boundary_without_moving_child_properties() {
        let property_before = View::text("x").padding(1).container();
        let property_after = View::text("x").container().padding(1);

        let ViewKind::Container(first) = property_before.kind() else {
            panic!("expected container");
        };
        let ViewKind::Container(second) = property_after.kind() else {
            panic!("expected container");
        };
        assert_eq!(first.child.decoration().padding, Insets::all(1));
        assert_eq!(property_before.decoration(), &Decoration::default());
        assert_eq!(second.child.decoration(), &Decoration::default());
        assert_eq!(property_after.decoration().padding, Insets::all(1));
    }

    #[test]
    fn structural_transforms_copy_width_and_remain_nested() {
        let inner_fill = View::text("x").fill_width().container();
        let outer_fill = View::text("x").container().fill_width();

        let ViewKind::Container(inner_fill_node) = inner_fill.kind() else {
            panic!("expected container");
        };
        let ViewKind::Container(outer_fill_node) = outer_fill.kind() else {
            panic!("expected container");
        };
        assert_eq!(inner_fill.width(), WidthRule::Fill);
        assert_eq!(inner_fill_node.child.width(), WidthRule::Fill);
        assert_eq!(outer_fill.width(), WidthRule::Fill);
        assert_eq!(outer_fill_node.child.width(), WidthRule::Fit);

        let inner_fill = View::text("x").fill_height().container();
        let outer_fill = View::text("x").container().fill_height();
        let ViewKind::Container(inner_fill_node) = inner_fill.kind() else {
            panic!("expected container");
        };
        let ViewKind::Container(outer_fill_node) = outer_fill.kind() else {
            panic!("expected container");
        };
        assert_eq!(
            inner_fill.height(),
            crate::presentation::ir::HeightRule::Fill
        );
        assert_eq!(
            inner_fill_node.child.height(),
            crate::presentation::ir::HeightRule::Fill
        );
        assert_eq!(
            outer_fill.height(),
            crate::presentation::ir::HeightRule::Fill
        );
        assert_eq!(
            outer_fill_node.child.height(),
            crate::presentation::ir::HeightRule::Fit
        );

        let nested = View::text("x").container().container();
        let ViewKind::Container(outer) = nested.kind() else {
            panic!("expected outer container");
        };
        assert!(matches!(outer.child.kind(), ViewKind::Container(_)));
    }

    #[test]
    fn clamp_is_a_structural_transform_with_a_copied_width() {
        let view = View::text("x")
            .fill_width()
            .clamp_rows(2, OverflowIndicator::None);
        let ViewKind::ClampRows(clamp) = view.kind() else {
            panic!("expected clamp");
        };

        assert_eq!(view.width(), WidthRule::Fill);
        assert_eq!(clamp.child.width(), WidthRule::Fill);
        assert_eq!(view.decoration(), &Decoration::default());
        assert!(matches!(clamp.child.kind(), ViewKind::Text(_)));
    }

    #[test]
    fn text_properties_keep_the_typed_boundary_until_structural_transform() {
        fn accepts_text(_: Text) {}

        let text = View::text("x")
            .padding(1)
            .background(ColorSpec::ansi(1))
            .foreground(ColorSpec::ansi(2))
            .border(BorderSpec::plain())
            .bold()
            .dim()
            .italic()
            .underline()
            .reversed()
            .text_attribute(TextAttribute::Bold, false)
            .no_wrap()
            .text_align(HorizontalAlign::End)
            .fill_width();
        accepts_text(text.clone());

        let view = text.clone().into_view();
        let ViewKind::Text(text_view) = view.kind() else {
            panic!("expected text");
        };
        assert_eq!(text_view.wrap, WrapMode::NoWrap);
        assert_eq!(view.width(), WidthRule::Fill);
        let view = text.container();
        assert!(matches!(view.kind(), ViewKind::Container(_)));
    }
}
