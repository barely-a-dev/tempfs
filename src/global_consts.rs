#[cfg(feature = "rand_gen")]
/// Number of retries to find a unique name for randomly generated temporary names.
pub const NUM_RETRY: usize = 1 << 32;

#[cfg(feature = "rand_gen")]
/// Length of randomly generated temporary names to generate.
pub const RAND_FN_LEN: usize = 16;

#[cfg(feature = "rand_gen")]
/// Valid characters which can be in randomly generated temporary names.
pub const VALID_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_";
