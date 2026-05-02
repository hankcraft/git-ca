use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use crate::error::{Error, Result};

/// Codex stamps the ChatGPT account id into the OIDC `id_token` under this
/// custom claim. We need it to set the `ChatGPT-Account-ID` header on chat
/// requests.
const CHATGPT_ACCOUNT_ID_CLAIM: &str = "https://api.openai.com/auth";

/// Extract `chatgpt_account_id` from an OIDC `id_token` JWT.
///
/// We only decode the payload segment — signature is not verified because
/// codex's auth flow already enforces transport security and the token came
/// from us exchanging our own PKCE code. Returns `Ok(None)` if the claim is
/// absent (the user's ChatGPT account is not linked).
pub fn chatgpt_account_id(id_token: &str) -> Result<Option<String>> {
    let payload = id_token
        .split('.')
        .nth(1)
        .ok_or_else(|| Error::CodexLogin("id_token is not a JWT".into()))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| Error::CodexLogin(format!("id_token payload not base64: {e}")))?;
    let json: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| Error::CodexLogin(format!("id_token payload not JSON: {e}")))?;
    let auth = json.get(CHATGPT_ACCOUNT_ID_CLAIM);
    let id = auth
        .and_then(|v| v.get("chatgpt_account_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt_with_payload(payload: &serde_json::Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\"}");
        let body = URL_SAFE_NO_PAD.encode(serde_json::to_vec(payload).unwrap());
        let sig = URL_SAFE_NO_PAD.encode(b"sig");
        format!("{header}.{body}.{sig}")
    }

    #[test]
    fn extracts_account_id_when_present() {
        let token = jwt_with_payload(&serde_json::json!({
            "sub": "user_123",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct_abc",
            },
        }));

        assert_eq!(
            chatgpt_account_id(&token).unwrap().as_deref(),
            Some("acct_abc")
        );
    }

    #[test]
    fn returns_none_when_claim_missing() {
        let token = jwt_with_payload(&serde_json::json!({ "sub": "user_123" }));
        assert!(chatgpt_account_id(&token).unwrap().is_none());
    }

    #[test]
    fn malformed_jwt_returns_error() {
        let err = chatgpt_account_id("not-a-jwt").unwrap_err();
        assert!(matches!(err, Error::CodexLogin(_)));
    }
}
