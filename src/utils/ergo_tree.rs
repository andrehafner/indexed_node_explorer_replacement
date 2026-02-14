//! ErgoTree utilities for address derivation and template hashing
//!
//! This module provides basic address derivation from ErgoTree hex strings.
//! For full compatibility, consider using the ergo-lib crate.

use blake2::{Blake2b, Digest, digest::consts::U32};

type Blake2b256 = Blake2b<U32>;

const MAINNET_P2PK_PREFIX: u8 = 0x01;  // P2PK address (network 0x00 + type 0x01)
const MAINNET_P2SH_PREFIX: u8 = 0x02;  // P2SH address (network 0x00 + type 0x02)
const MAINNET_P2S_PREFIX: u8 = 0x03;   // P2S address  (network 0x00 + type 0x03)

const TESTNET_P2PK_PREFIX: u8 = 0x11;  // network 0x10 + type 0x01
const TESTNET_P2SH_PREFIX: u8 = 0x12;  // network 0x10 + type 0x02
const TESTNET_P2S_PREFIX: u8 = 0x13;   // network 0x10 + type 0x03

/// Base58 alphabet used by Ergo
const BASE58_ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Convert ErgoTree hex to human-readable address
pub fn ergo_tree_to_address(ergo_tree: &str) -> Option<String> {
    let bytes = hex::decode(ergo_tree).ok()?;

    if bytes.is_empty() {
        return None;
    }

    // Check for P2PK tree (starts with 0008cd)
    if bytes.len() >= 36 && bytes[0] == 0x00 && bytes[1] == 0x08 && bytes[2] == 0xcd {
        // Extract public key (33 bytes after prefix)
        let pk = &bytes[3..36];
        return Some(encode_p2pk_address(pk, true));
    }

    // For other trees, use P2S encoding
    Some(encode_p2s_address(&bytes, true))
}

/// Encode a P2PK address from public key bytes
fn encode_p2pk_address(pk: &[u8], mainnet: bool) -> String {
    let prefix = if mainnet { MAINNET_P2PK_PREFIX } else { TESTNET_P2PK_PREFIX };

    let mut content = vec![prefix];
    content.extend_from_slice(pk);

    // Add checksum (first 4 bytes of blake2b256 hash)
    let checksum = blake2b256_checksum(&content);
    content.extend_from_slice(&checksum);

    base58_encode(&content)
}

/// Encode a P2S address from ErgoTree bytes
///
/// P2S addresses use the full ErgoTree bytes directly (not a hash).
/// Format: prefix_byte || ergo_tree_bytes || checksum(4 bytes)
fn encode_p2s_address(tree: &[u8], mainnet: bool) -> String {
    let prefix = if mainnet { MAINNET_P2S_PREFIX } else { TESTNET_P2S_PREFIX };

    let mut content = vec![prefix];
    content.extend_from_slice(tree);

    // Add checksum (first 4 bytes of blake2b256 of content)
    let checksum = blake2b256_checksum(&content);
    content.extend_from_slice(&checksum);

    base58_encode(&content)
}

/// Compute Blake2b256 hash
fn blake2b256(data: &[u8]) -> Vec<u8> {
    let mut hasher = Blake2b256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Compute checksum (first 4 bytes of blake2b256)
fn blake2b256_checksum(data: &[u8]) -> Vec<u8> {
    blake2b256(data)[..4].to_vec()
}

/// Base58 encode bytes
fn base58_encode(data: &[u8]) -> String {
    // Count leading zeros
    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();

    // Convert to base58
    let mut result = Vec::new();
    let mut num = data.to_vec();

    while !num.is_empty() && !(num.len() == 1 && num[0] == 0) {
        let mut remainder = 0u32;
        let mut new_num = Vec::new();

        for &byte in &num {
            let acc = (remainder << 8) + byte as u32;
            let quotient = acc / 58;
            remainder = acc % 58;

            if !new_num.is_empty() || quotient > 0 {
                new_num.push(quotient as u8);
            }
        }

        result.push(BASE58_ALPHABET[remainder as usize]);
        num = new_num;
    }

    // Add leading '1's for leading zeros
    for _ in 0..leading_zeros {
        result.push(b'1');
    }

    result.reverse();
    String::from_utf8(result).unwrap_or_default()
}

/// Compute ErgoTree template hash
/// This extracts the "template" of an ErgoTree by replacing constants with placeholders
pub fn ergo_tree_template_hash(ergo_tree: &str) -> String {
    // For simplicity, we hash the first 8 bytes + structure
    // A full implementation would parse and extract the template
    let bytes = match hex::decode(ergo_tree) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };

    if bytes.is_empty() {
        return String::new();
    }

    // Simple template: use first byte (header) + size as template indicator
    // This is a simplified version - full implementation needs ErgoTree parsing
    let template_bytes = if bytes.len() > 8 {
        &bytes[..8]
    } else {
        &bytes
    };

    let hash = blake2b256(template_bytes);
    hex::encode(&hash[..32])
}

/// Convert miner public key to address
pub fn miner_pk_to_address(miner_pk: &str) -> Option<String> {
    let pk_bytes = hex::decode(miner_pk).ok()?;

    if pk_bytes.len() != 33 {
        return None;
    }

    Some(encode_p2pk_address(&pk_bytes, true))
}

/// Validate an Ergo address
pub fn validate_address(address: &str) -> bool {
    // P2PK addresses are ~51 chars, P2S addresses can be hundreds of chars
    if address.len() < 30 {
        return false;
    }

    // Check all characters are valid base58
    address.chars().all(|c| BASE58_ALPHABET.contains(&(c as u8)))
}

/// Get address type from prefix
pub fn get_address_type(address: &str) -> Option<&'static str> {
    if address.is_empty() {
        return None;
    }

    match address.chars().next()? {
        '9' => Some("P2PK"),  // Mainnet P2PK
        '2' => Some("P2S"),   // Mainnet P2S
        '3' => Some("P2SH"),  // Mainnet P2SH
        _ => Some("Unknown"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p2pk_address() {
        // Known P2PK ErgoTree
        let ergo_tree = "0008cd03a1e7be27b2f0e4a6e4f6f3e3e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4";
        let address = ergo_tree_to_address(ergo_tree);
        assert!(address.is_some());
        assert!(address.unwrap().starts_with('9'));
    }

    #[test]
    fn test_p2s_address_known_vector() {
        // Test vector from sigma-rust reference implementation
        let ergo_tree = "100204a00b08cd021dde34603426402615658f1d970cfa7c7bd92ac81a8b16eeebff264d59ce4604ea02d192a39a8cc7a70173007301";
        let address = ergo_tree_to_address(ergo_tree).unwrap();
        assert_eq!(
            address,
            "88dhgzEuTXaQLG2u9aud6SkPCGyXvw8mQWLCWfkv6wwuC9X9gdzELR9mt2hHQaM654aamzscP8r45NsJ",
            "P2S address must match sigma-rust reference"
        );
    }

    #[test]
    fn test_p2s_miners_fee_address() {
        // Miners fee ErgoTree - known test vector from sigma-rust
        let ergo_tree = "1005040004000e36100204a00b08cd0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798ea02d192a39a8cc7a701730073011001020402d19683030193a38cc7b2a57300000193c2b2a57301007473027303830108cdeeac93b1a57304";
        let address = ergo_tree_to_address(ergo_tree).unwrap();
        assert_eq!(
            address,
            "2iHkR7CWvD1R4j1yZg5bkeDRQavjAaVPeTDFGGLZduHyfWMuYpmhHocX8GJoaieTx78FntzJbCBVL6rf96ocJoZdmWBL2fci7NqWgAirppPQmZ7fN9V6z13Ay6brPriBKYqLp1bT2Fk4FkFLCfdPpe",
            "Miners fee P2S address must match sigma-rust reference"
        );
    }

    #[test]
    fn test_validate_address() {
        assert!(validate_address("9fRAWhdxEsTcdb8PhGNrZfwqa65zfkuYHAMmkQLcic1gdLSV5vA"));
        // P2S addresses can be very long
        assert!(validate_address("BxKBaHkvrTvLZrDcZjcsxsF7aSsrN73ijeFZXtbj4CXZHHcvBtqSxQ"));
        assert!(!validate_address("invalid"));
        assert!(!validate_address("0invalid"));
    }
}
