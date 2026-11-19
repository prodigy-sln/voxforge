//! The two namespaced names VoxForge introduces.
//!
//! Both are newtypes over [`NamespacedId`], deliberately not interchangeable
//! with each other or with [`TextureKey`](crate::shared::id::TextureKey): handing
//! one where another is expected is a compile error.

use crate::shared::id::{NamespacedId, NamespacedIdError};

/// The namespaced key a material is declared under, such as `base:oak_plank`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialKey(NamespacedId);

impl MaterialKey {
    /// Parses `text` as a material key.
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

/// The namespaced name a model document declares, such as `base:torch`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModelName(NamespacedId);

impl ModelName {
    /// Parses `text` as a model name.
    ///
    /// # Errors
    ///
    /// Returns [`NamespacedIdError`] if `text` is not `namespace:path` with both
    /// sides non-empty.
    pub fn parse(text: &str) -> Result<Self, NamespacedIdError> {
        Ok(Self(NamespacedId::parse(text)?))
    }

    /// The name exactly as it was written.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
