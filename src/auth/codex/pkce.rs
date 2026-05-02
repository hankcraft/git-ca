use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// PKCE pair generated for a single OAuth authorize attempt.
///
/// Verifier is 64 random bytes encoded as URL-safe base64 (no padding) — the
/// shape codex itself uses, which the OpenAI authorize endpoint accepts.
pub struct Pkce {
    pub code_verifier: String,
    pub code_challenge: String,
}

pub fn generate() -> Result<Pkce> {
    let mut bytes = [0u8; 64];
    getrandom::getrandom(&mut bytes).map_err(|e| Error::CodexLogin(format!("rng failed: {e}")))?;
    let code_verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge_digest = Sha256::digest(code_verifier.as_bytes());
    let code_challenge = URL_SAFE_NO_PAD.encode(challenge_digest);
    Ok(Pkce {
        code_verifier,
        code_challenge,
    })
}

/// Generate a URL-safe random token used for OAuth `state` and similar.
pub fn random_token(byte_len: usize) -> Result<String> {
    let mut bytes = vec![0u8; byte_len];
    getrandom::getrandom(&mut bytes).map_err(|e| Error::CodexLogin(format!("rng failed: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_is_sha256_of_verifier() {
        let pkce = generate().unwrap();
        let recomputed = URL_SAFE_NO_PAD.encode(Sha256::digest(pkce.code_verifier.as_bytes()));
        assert_eq!(pkce.code_challenge, recomputed);
    }

    #[test]
    fn verifier_length_matches_64_byte_base64_no_pad() {
        let pkce = generate().unwrap();
        // 64 bytes → ceil(64*4/3) = 86 chars with no padding.
        assert_eq!(pkce.code_verifier.len(), 86);
    }

    #[test]
    fn random_tokens_differ_across_calls() {
        let a = random_token(32).unwrap();
        let b = random_token(32).unwrap();
        assert_ne!(a, b);
    }
}
