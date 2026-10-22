//! Cap (`capjs-core`) proof-of-work solver.
//!
//! The site gates `POST /api/auth/login` behind `/api/auth/captcha/challenge` +
//! `/api/auth/captcha/redeem`. Both endpoints are Cloudflare-protected POSTs, and
//! the challenge is a self-contained HMAC-signed JWT, so a client only needs the
//! documented PRNG + SHA-256 loop to obtain a redeemable captcha token.
//!
//! Verified against the shipped `capjs-core@0.1.6` sources:
//! `prng.js` (FNV-1a + xorshift32) and `index.js::validateChallenge`
//! (per-index salt/target derivation, `sha256(salt + nonce)` prefix match).

use crate::util::{fnv1a, fnv1a_resume, prng_from_hash};
use crate::Error;
use sha2::{Digest, Sha256};

/// Shape of the payload returned by `POST /api/auth/captcha/challenge`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ChallengeResponse {
    pub challenge: ChallengeParams,
    pub token: String,
    #[serde(default)]
    pub expires: Option<i64>,
    /// Present when the deployment enables the browser instrumentation probe.
    #[serde(default)]
    pub instrumentation: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
pub struct ChallengeParams {
    /// Challenge count.
    pub c: u32,
    /// Salt length in hex characters.
    pub s: u32,
    /// Difficulty in hex nibbles of the required hash prefix.
    pub d: u32,
}

/// Derives the salt and target prefix for challenge index `index` (1-based).
pub fn derive_salt_and_target(
    token: &str,
    index: u32,
    size: u32,
    difficulty: u32,
) -> (String, String) {
    let token_fnv = fnv1a(token);
    let salt_seed = fnv1a_resume(token_fnv, &index.to_string());
    let target_seed = fnv1a_resume(salt_seed, "d");
    (
        prng_from_hash(salt_seed, size as usize),
        prng_from_hash(target_seed, difficulty as usize),
    )
}

/// Returns the smallest nonce whose `sha256(salt + nonce)` hex digest starts with
/// `target`. Difficulty 4 (the site default) averages ~65k hashes per challenge.
pub fn solve_one(salt: &str, target: &str) -> u64 {
    let prefix = target.as_bytes();
    let mut nonce: u64 = 0;
    loop {
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(nonce.to_string().as_bytes());
        let digest = hasher.finalize();
        let hex = hex::encode(digest);
        if hex.as_bytes().starts_with(prefix) {
            return nonce;
        }
        nonce += 1;
    }
}

/// Solves every sub-challenge of a challenge token, in index order.
///
/// The sub-challenges are independent, so they are spread over the available
/// cores: the site ships `c = 50, d = 4`, i.e. ~50 x 65k hashes, which is ~0.5 s
/// single-threaded and well under the 10-minute challenge TTL.
pub fn solve(token: &str, params: ChallengeParams) -> Vec<u64> {
    let count = params.c as usize;
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, count.max(1));
    if count == 0 {
        return Vec::new();
    }
    let mut solutions = vec![0u64; count];
    std::thread::scope(|scope| {
        let chunk = count.div_ceil(workers);
        for (chunk_index, slot) in solutions.chunks_mut(chunk).enumerate() {
            let base = chunk_index * chunk;
            scope.spawn(move || {
                for (offset, cell) in slot.iter_mut().enumerate() {
                    let index = (base + offset + 1) as u32;
                    let (salt, target) = derive_salt_and_target(token, index, params.s, params.d);
                    *cell = solve_one(&salt, &target);
                }
            });
        }
    });
    solutions
}

/// Validates a solution set the way `validateChallenge` does, without needing the
/// HMAC secret: recomputes each salt/target from the token and checks the prefix.
pub fn verify(token: &str, params: ChallengeParams, solutions: &[u64]) -> bool {
    if solutions.len() != params.c as usize {
        return false;
    }
    solutions.iter().enumerate().all(|(i, nonce)| {
        let (salt, target) = derive_salt_and_target(token, i as u32 + 1, params.s, params.d);
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(nonce.to_string().as_bytes());
        let hex = hex::encode(hasher.finalize());
        hex.starts_with(&target)
    })
}

/// Decodes the JWT payload of a challenge token (no signature verification; the
/// server re-derives everything from the signed token itself).
pub fn decode_payload(token: &str) -> Result<serde_json::Value, Error> {
    let mut parts = token.split('.');
    let _header = parts
        .next()
        .ok_or_else(|| Error::Captcha("malformed token".into()))?;
    let payload = parts
        .next()
        .ok_or_else(|| Error::Captcha("malformed token".into()))?;
    let bytes = base64url_decode(payload)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| Error::Captcha(format!("payload decode failed: {e}")))
}

fn base64url_decode(input: &str) -> Result<Vec<u8>, Error> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut lookup = [255u8; 256];
    for (i, c) in TABLE.iter().enumerate() {
        lookup[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(input.len() * 3 / 4 + 3);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    for byte in input.bytes() {
        if byte == b'=' {
            break;
        }
        let value = lookup[byte as usize];
        if value == 255 {
            return Err(Error::Captcha("invalid base64url character".into()));
        }
        buffer = (buffer << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = concat!(
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.",
        "eyJuIjoiODI0MGIxNmQwN2RiNWVhNzY2OTMxMDVmMjUyM2ZlZDY3MDBkNTM4YzQ4YTc2NzlhYTEiLCJjIjo1MCwicyI6MzIsImQiOjQsImV4cCI6MTc5MjY3ODQ4NjY2MywiaWF0IjoxNzkyNjc3ODg2NjYzLCJzayI6Imt1bi1jYXB0Y2hhIn0.",
        "WA-iRUHn_Xpr1fBM-Pt1xJt8idYb-huE8GMrRe4SnHU"
    );

    #[test]
    fn decodes_real_challenge_payload() {
        let payload = decode_payload(TOKEN).expect("payload");
        assert_eq!(payload["c"], 50);
        assert_eq!(payload["s"], 32);
        assert_eq!(payload["d"], 4);
        assert_eq!(payload["sk"], "kun-captcha");
    }

    #[test]
    fn reproduces_reference_nonce() {
        // Browser run of the same challenge produced nonce 55266 for index 1.
        let (salt, target) = derive_salt_and_target(TOKEN, 1, 32, 4);
        assert_eq!(salt, "509f38104431181e816f9b67326b0ea6");
        assert_eq!(target, "2083");
        assert_eq!(solve_one(&salt, &target), 55266);
    }

    #[test]
    fn solves_and_verifies_full_challenge() {
        let params = ChallengeParams { c: 4, s: 32, d: 4 };
        let solutions = solve(TOKEN, params);
        assert_eq!(solutions.len(), 4);
        assert!(verify(TOKEN, params, &solutions));
        assert!(!verify(TOKEN, params, &solutions[..3]));
    }
}
