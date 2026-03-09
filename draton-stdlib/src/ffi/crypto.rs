use sha2::{Digest, Sha256, Sha512};
use uuid::Uuid;

fn fill_random(bytes: &mut [u8]) {
    let _ = getrandom::getrandom(bytes);
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    fill_random(&mut bytes);
    u64::from_le_bytes(bytes)
}

/// Returns the SHA-256 digest as lowercase hex.
pub fn sha256(data: impl AsRef<str>) -> String {
    encode_hex(&Sha256::digest(data.as_ref().as_bytes()))
}

/// Returns the SHA-512 digest as lowercase hex.
pub fn sha512(data: impl AsRef<str>) -> String {
    encode_hex(&Sha512::digest(data.as_ref().as_bytes()))
}

/// Returns the MD5 digest as lowercase hex.
pub fn md5(data: impl AsRef<str>) -> String {
    format!("{:x}", md5::compute(data.as_ref().as_bytes()))
}

/// Returns a random UUID v4 string.
pub fn uuid() -> String {
    Uuid::new_v4().to_string()
}

/// Returns `n` random bytes.
pub fn random_bytes(n: i64) -> Vec<u8> {
    if n <= 0 {
        return Vec::new();
    }
    let mut bytes = vec![0u8; n as usize];
    fill_random(&mut bytes);
    bytes
}

/// Returns a random integer in the inclusive range `[min, max]`.
pub fn random_int(min: i64, max: i64) -> i64 {
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    let span = (hi as i128 - lo as i128 + 1) as u128;
    if span == (u64::MAX as u128) + 1 {
        return i64::from_le_bytes(random_u64().to_le_bytes());
    }
    let span = span as u64;
    let cutoff = u64::MAX - (u64::MAX % span);
    loop {
        let candidate = random_u64();
        if candidate < cutoff {
            return lo + (candidate % span) as i64;
        }
    }
}
