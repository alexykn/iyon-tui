use std::sync::Arc;

use super::{Annotations, LiteralText, TextIrError, TextRun};

/// Inline line-break semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BreakKind {
    Soft,
    Hard,
}

/// Format identifier for a literal embedded language.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FormatId(Arc<str>);

impl FormatId {
    pub fn new(value: impl Into<Arc<str>>) -> Result<Self, TextIrError> {
        let value = value.into();
        super::errors::validate_name(&value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Language identifier used for nested code projectors.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageId(Arc<str>);

impl LanguageId {
    pub fn new(value: impl Into<Arc<str>>) -> Result<Self, TextIrError> {
        let value = value.into();
        super::errors::validate_name(&value)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A resolved link target.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkTarget {
    destination: Arc<str>,
    title: Option<Arc<str>>,
}

impl LinkTarget {
    pub fn new(destination: impl Into<Arc<str>>, title: Option<impl Into<Arc<str>>>) -> Self {
        Self {
            destination: destination.into(),
            title: title.map(Into::into),
        }
    }

    #[must_use]
    pub fn destination(&self) -> &str {
        &self.destination
    }
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

/// A generic inline formatting mark.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mark {
    Emphasis,
    Strong,
    Strikethrough,
    Underline,
    Superscript,
    Subscript,
    SmallCaps,
    Code,
    Link(LinkTarget),
}

/// Canonical, order-independent inline marks.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MarkSet(Arc<[Mark]>);

impl MarkSet {
    pub fn new(marks: impl IntoIterator<Item = Mark>) -> Result<Self, TextIrError> {
        let mut marks: Vec<_> = marks.into_iter().collect();
        marks.sort();
        marks.dedup();
        if marks
            .iter()
            .filter(|mark| matches!(mark, Mark::Link(_)))
            .count()
            > 1
        {
            return Err(TextIrError::DuplicateLinkMark);
        }
        Ok(Self(marks.into()))
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn marks(&self) -> &[Mark] {
        &self.0
    }

    pub fn with_mark(&self, mark: Mark) -> Result<Self, TextIrError> {
        let mut marks = self.0.to_vec();
        marks.push(mark);
        Self::new(marks)
    }

    #[must_use]
    pub fn contains(&self, mark: &Mark) -> bool {
        self.0.binary_search(mark).is_ok()
    }
}

/// Immutable ordered inline content.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct InlineContent {
    items: Arc<[Inline]>,
}

impl From<Inline> for InlineContent {
    fn from(value: Inline) -> Self {
        Self::new([value])
    }
}

impl From<TextRun> for InlineContent {
    fn from(value: TextRun) -> Self {
        Self::new([Inline::text(value)])
    }
}

impl From<&str> for InlineContent {
    fn from(value: &str) -> Self {
        Self::from(TextRun::from(value))
    }
}

impl From<String> for InlineContent {
    fn from(value: String) -> Self {
        Self::from(TextRun::from(value))
    }
}

impl FromIterator<Inline> for InlineContent {
    fn from_iter<T: IntoIterator<Item = Inline>>(iter: T) -> Self {
        Self::new(iter)
    }
}

impl InlineContent {
    pub fn new(items: impl IntoIterator<Item = Inline>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn items(&self) -> &[Inline] {
        &self.items
    }
    pub fn iter(&self) -> impl Iterator<Item = &Inline> {
        self.items.iter()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn with_mark(&self, mark: Mark) -> Result<Self, TextIrError> {
        self.items
            .iter()
            .map(|inline| inline.with_mark(mark.clone()))
            .collect::<Result<Vec<_>, _>>()
            .map(Self::new)
    }

    #[must_use]
    pub fn strong(&self) -> Self {
        self.with_mark(Mark::Strong)
            .expect("Strong is a valid mark")
    }

    #[must_use]
    pub fn emphasis(&self) -> Self {
        self.with_mark(Mark::Emphasis)
            .expect("Emphasis is a valid mark")
    }

    #[must_use]
    pub fn code(&self) -> Self {
        self.with_mark(Mark::Code).expect("Code is a valid mark")
    }
}

/// Generic inline semantic kind.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InlineKind {
    Text(TextRun),
    Break(BreakKind),
    Image(Image),
    RawInline { format: FormatId, body: LiteralText },
}

/// Immutable inline semantic value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inline(Arc<InlineData>);

#[derive(Clone, Debug, PartialEq, Eq)]
struct InlineData {
    kind: InlineKind,
    marks: MarkSet,
    annotations: Annotations,
}

impl Inline {
    #[must_use]
    pub fn new(kind: InlineKind) -> Self {
        Self(Arc::new(InlineData {
            kind,
            marks: MarkSet::default(),
            annotations: Annotations::default(),
        }))
    }

    pub fn text(run: impl Into<TextRun>) -> Self {
        Self::new(InlineKind::Text(run.into()))
    }
    #[must_use]
    pub fn break_(kind: BreakKind) -> Self {
        Self::new(InlineKind::Break(kind))
    }
    #[must_use]
    pub fn image(image: Image) -> Self {
        Self::new(InlineKind::Image(image))
    }
    #[must_use]
    pub fn raw(format: FormatId, body: LiteralText) -> Self {
        Self::new(InlineKind::RawInline { format, body })
    }

    #[must_use]
    pub fn kind(&self) -> &InlineKind {
        &self.0.kind
    }
    #[must_use]
    pub fn marks(&self) -> &MarkSet {
        &self.0.marks
    }
    #[must_use]
    pub fn annotations(&self) -> &Annotations {
        &self.0.annotations
    }

    #[must_use]
    pub fn as_text(&self) -> Option<&TextRun> {
        match &self.0.kind {
            InlineKind::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn with_mark(&self, mark: Mark) -> Result<Self, TextIrError> {
        Ok(self.with_marks(self.marks().with_mark(mark)?))
    }

    #[must_use]
    pub fn strong(&self) -> Self {
        self.with_mark(Mark::Strong)
            .expect("Strong is a valid mark")
    }

    #[must_use]
    pub fn emphasis(&self) -> Self {
        self.with_mark(Mark::Emphasis)
            .expect("Emphasis is a valid mark")
    }

    #[must_use]
    pub fn strikethrough(&self) -> Self {
        self.with_mark(Mark::Strikethrough)
            .expect("Strikethrough is a valid mark")
    }

    #[must_use]
    pub fn underline(&self) -> Self {
        self.with_mark(Mark::Underline)
            .expect("Underline is a valid mark")
    }

    #[must_use]
    pub fn code(&self) -> Self {
        self.with_mark(Mark::Code).expect("Code is a valid mark")
    }

    pub fn with_link(&self, target: LinkTarget) -> Result<Self, TextIrError> {
        self.with_mark(Mark::Link(target))
    }

    #[must_use]
    pub fn with_marks(&self, marks: MarkSet) -> Self {
        Self(Arc::new(InlineData {
            kind: self.0.kind.clone(),
            marks,
            annotations: self.0.annotations.clone(),
        }))
    }

    #[must_use]
    pub fn with_annotations(&self, annotations: Annotations) -> Self {
        Self(Arc::new(InlineData {
            kind: self.0.kind.clone(),
            marks: self.0.marks.clone(),
            annotations,
        }))
    }
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    #[must_use]
    pub fn map_annotations(&self, map: impl FnOnce(Annotations) -> Annotations) -> Self {
        self.with_annotations(map(self.annotations().clone()))
    }

    pub(crate) fn from_parts(kind: InlineKind, marks: MarkSet, annotations: Annotations) -> Self {
        Self(Arc::new(InlineData {
            kind,
            marks,
            annotations,
        }))
    }
}

/// A terminal image value with semantic alt content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    destination: Arc<str>,
    title: Option<Arc<str>>,
    alt: InlineContent,
}

impl Image {
    pub fn new(
        destination: impl Into<Arc<str>>,
        title: Option<impl Into<Arc<str>>>,
        alt: impl Into<InlineContent>,
    ) -> Self {
        Self {
            destination: destination.into(),
            title: title.map(Into::into),
            alt: alt.into(),
        }
    }

    #[must_use]
    pub fn destination(&self) -> &str {
        &self.destination
    }
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
    #[must_use]
    pub fn alt(&self) -> &InlineContent {
        &self.alt
    }
}
