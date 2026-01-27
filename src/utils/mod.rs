pub mod ergo_tree;

use sha2::{Digest, Sha256};
use blake2::{Blake2b, digest::consts::U32};

type Blake2b256 = Blake2b<U32>;

/// Compute Blake2b256 hash
pub fn blake2b256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Blake2b256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Compute SHA256 hash
pub fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Encode bytes as hex string
pub fn to_hex(data: &[u8]) -> String {
    hex::encode(data)
}

/// Decode hex string to bytes
pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    hex::decode(s).ok()
}

/// Paginate a vector
pub fn paginate<T: Clone>(items: &[T], offset: usize, limit: usize) -> Vec<T> {
    items
        .iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect()
}

/// Parse i64 from string
pub fn parse_i64(s: &str) -> Option<i64> {
    s.parse().ok()
}
