#[cfg(feature = "rand_gen")]
pub const NUM_RETRY: usize = 1 << 24;

#[cfg(feature = "rand_gen")]
pub const RAND_FN_LEN: usize = 16;

#[cfg(feature = "rand_gen")]
pub(crate) const VALID_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_";
