//! Primitives the texture build relies on: namespaced ids, the FNV-1a fold and
//! the texture-set index format.

pub mod art;
pub mod hash;
pub mod id;

/// The edge of one block texture, in texels. `build` bakes every image to it.
pub const TEXTURE_EDGE: u32 = 16;
