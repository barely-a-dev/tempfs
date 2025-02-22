#[cfg(feature = "rand_gen")]
use once_cell::sync::OnceCell;

#[cfg(feature = "rand_gen")]
/// Number of retries to find a unique name for randomly generated temporary object names.
static NUM_RETRY: OnceCell<usize> = OnceCell::new();

/// Sets the number of retries to find a unique name for randomly generated temporary object names. Errors if run more than once or after any randomly named temporary object is created.
#[allow(dead_code)]
#[cfg(feature = "rand_gen")]
pub fn set_num_retry(val: usize) -> Result<(), &'static str> {
    NUM_RETRY
        .set(val)
        .map_err(|_| "NUM_RETRY has already been set")
}

/// Gets the number of retries to find a unique name for randomly generated temporary object names.
#[allow(dead_code)]
#[cfg(feature = "rand_gen")]
pub fn num_retry() -> usize {
    *NUM_RETRY.get_or_init(|| 1 << 32)
}

#[cfg(feature = "rand_gen")]
/// Length of randomly generated temporary object names to generate.
static RAND_FN_LEN: OnceCell<usize> = OnceCell::new();

/// Set the length of randomly generated temporary object names to generate. Errors if run more than once or after any randomly named temporary object is created.
#[allow(dead_code)]
#[cfg(feature = "rand_gen")]
pub fn set_rand_fn_len(val: usize) -> Result<(), &'static str> {
    RAND_FN_LEN
        .set(val)
        .map_err(|_| "RAND_FN_LEN has already been set")
}

/// Get the length of randomly generated temporary object names to generate.
#[cfg(feature = "rand_gen")]
pub fn rand_fn_len() -> usize {
    *RAND_FN_LEN.get_or_init(|| 16)
}

#[cfg(feature = "rand_gen")]
/// Valid characters which can be in randomly generated temporary object names.
static VALID_CHARS: OnceCell<&'static [u8]> = OnceCell::new();

/// Set the valid characters which can be in randomly generated temporary object names. Errors if run more than once or after any randomly named temporary object is created.
#[allow(dead_code)]
#[cfg(feature = "rand_gen")]
pub fn set_valid_chars(val: &'static [u8]) -> Result<(), &'static str> {
    VALID_CHARS
        .set(val)
        .map_err(|_| "VALID_CHARS has already been set")
}

/// Gets the valid characters which can be in randomly generated temporary object names.
#[cfg(feature = "rand_gen")]
pub fn valid_chars() -> &'static [u8] {
    VALID_CHARS.get_or_init(|| b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_")
}
