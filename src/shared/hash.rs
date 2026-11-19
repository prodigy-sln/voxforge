//! FNV-1a 64, the fold a texture-set index records.

/// Published FNV-1a 64 constants. Changing them changes every recorded fold.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// `bytes` folded with FNV-1a 64.
///
/// Hand-written rather than the standard library's hasher, whose algorithm is
/// unspecified and may change between compiler releases.
pub fn fnv_1a_64(bytes: &[u8]) -> u64 {
    let mut folded = FNV_OFFSET_BASIS;
    for byte in bytes {
        folded ^= u64::from(*byte);
        folded = folded.wrapping_mul(FNV_PRIME);
    }
    folded
}
