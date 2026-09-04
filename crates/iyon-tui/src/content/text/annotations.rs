use std::{fmt, sync::Arc};

use super::{TextIrError, errors::validate_name};

/// A namespaced semantic tag.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticTag {
    namespace: Arc<str>,
    name: Arc<str>,
}

impl SemanticTag {
    pub fn new(
        namespace: impl Into<Arc<str>>,
        name: impl Into<Arc<str>>,
    ) -> Result<Self, TextIrError> {
        let namespace = namespace.into();
        let name = name.into();
        validate_name(&namespace)?;
        validate_name(&name)?;
        Ok(Self { namespace, name })
    }

    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A namespaced semantic property key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticKey {
    namespace: Arc<str>,
    name: Arc<str>,
}

impl SemanticKey {
    pub fn new(
        namespace: impl Into<Arc<str>>,
        name: impl Into<Arc<str>>,
    ) -> Result<Self, TextIrError> {
        let namespace = namespace.into();
        let name = name.into();
        validate_name(&namespace)?;
        validate_name(&name)?;
        Ok(Self { namespace, name })
    }

    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Small, typed annotation values.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticValue {
    Bool(bool),
    Integer(i64),
    Text(Arc<str>),
    TextList(Arc<[Arc<str>]>),
}

impl From<bool> for SemanticValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
impl From<i64> for SemanticValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}
impl From<String> for SemanticValue {
    fn from(value: String) -> Self {
        Self::Text(value.into())
    }
}
impl From<&str> for SemanticValue {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

/// Immutable canonical semantic annotations.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Annotations {
    tags: Arc<[SemanticTag]>,
    properties: Arc<[(SemanticKey, SemanticValue)]>,
}

impl Annotations {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn tags(&self) -> &[SemanticTag] {
        &self.tags
    }

    #[must_use]
    pub fn properties(&self) -> &[(SemanticKey, SemanticValue)] {
        &self.properties
    }

    #[must_use]
    pub fn add_tag(&self, tag: SemanticTag) -> Self {
        self.clone().with_tag(tag)
    }

    #[must_use]
    pub fn with_tag(mut self, tag: SemanticTag) -> Self {
        if !self.tags.iter().any(|existing| existing == &tag) {
            let mut tags = self.tags.to_vec();
            tags.push(tag);
            tags.sort();
            self.tags = tags.into();
        }
        self
    }

    #[must_use]
    pub fn set_property(&self, key: SemanticKey, value: impl Into<SemanticValue>) -> Self {
        self.clone().with_property(key, value)
    }

    #[must_use]
    pub fn with_property(mut self, key: SemanticKey, value: impl Into<SemanticValue>) -> Self {
        let value = value.into();
        let mut properties = self.properties.to_vec();
        if let Some(existing) = properties.iter_mut().find(|(existing, _)| existing == &key) {
            existing.1 = value;
        } else {
            properties.push((key, value));
        }
        properties.sort_by(|(left, _), (right, _)| left.cmp(right));
        self.properties = properties.into();
        self
    }

    #[must_use]
    pub fn contains_tag(&self, tag: &SemanticTag) -> bool {
        self.tags.binary_search(tag).is_ok()
    }

    #[must_use]
    pub fn property(&self, key: &SemanticKey) -> Option<&SemanticValue> {
        self.properties
            .binary_search_by(|(existing, _)| existing.cmp(key))
            .ok()
            .map(|index| &self.properties[index].1)
    }
}

impl fmt::Display for SemanticTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.name)
    }
}

impl fmt::Display for SemanticKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.name)
    }
}
