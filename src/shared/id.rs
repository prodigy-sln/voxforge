//! The `namespace:path` id that material keys, model names and texture keys share.

use std::sync::Arc;

use thiserror::Error;

/// Why a `namespace:path` id could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NamespacedIdError {
    #[error("`{text}` has no namespace — a namespaced id is written `namespace:path`")]
    MissingNamespace { text: String },
    #[error("`{text}` has an empty namespace")]
    EmptyNamespace { text: String },
    #[error("`{text}` has an empty path")]
    EmptyPath { text: String },
    #[error(
        "`{text}` has more than one namespace separator — a namespaced id is written `namespace:path`"
    )]
    MultipleSeparators { text: String },
}

/// A validated `namespace:path` id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NamespacedId(Arc<str>);

impl NamespacedId {
    /// Exactly one separator with both sides non-empty. No character set is
    /// imposed.
    ///
    /// Exactly one, not at least one: splitting on the first separator would
    /// turn `example:granite:top` into the path `granite:top`, so a typo would
    /// become a plausible-looking id that resolves to nothing.
    ///
    /// # Errors
    ///
    /// Returns [`NamespacedIdError`] if `text` is not `namespace:path` with both
    /// sides non-empty.
    pub fn parse(text: &str) -> Result<Self, NamespacedIdError> {
        let Some((namespace, path)) = text.split_once(':') else {
            return Err(NamespacedIdError::MissingNamespace {
                text: text.to_owned(),
            });
        };
        if path.contains(':') {
            return Err(NamespacedIdError::MultipleSeparators {
                text: text.to_owned(),
            });
        }
        if namespace.is_empty() {
            return Err(NamespacedIdError::EmptyNamespace {
                text: text.to_owned(),
            });
        }
        if path.is_empty() {
            return Err(NamespacedIdError::EmptyPath {
                text: text.to_owned(),
            });
        }
        Ok(Self(Arc::from(text)))
    }

    /// The id exactly as it was written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The namespaced key a texture is named by, such as `example:granite`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureKey(NamespacedId);

impl TextureKey {
    /// Parses `text` as a texture key.
    ///
    /// # Errors
    ///
    /// Returns [`NamespacedIdError`] if `text` is not `namespace:path` with both
    /// sides non-empty.
    pub fn parse(text: &str) -> Result<Self, NamespacedIdError> {
        Ok(Self(NamespacedId::parse(text)?))
    }

    /// The key exactly as it was written.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
