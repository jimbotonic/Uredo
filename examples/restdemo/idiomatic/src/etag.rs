//! Entity tags, so a client that already has the answer does not receive it again.

const OFFSET: u64 = 0xcbf29ce484222325;
const PRIME: u64 = 0x100000001b3;

/// The tag for a representation, quoted as HTTP wants it.
pub fn of(body: &[u8]) -> String {
    let mut hash = OFFSET;
    for byte in body {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    format!("\"{hash:016x}\"")
}

/// Does the client already hold this representation?
pub fn matches(if_none_match: &str, tag: &str) -> bool {
    if_none_match
        .split(',')
        .map(str::trim)
        .any(|candidate| candidate == "*" || candidate == tag)
}
