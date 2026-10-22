//! Login: captcha challenge -> PoW solve -> redeem -> `POST /api/auth/login`.
//!
//! Observed flow on the live site (`components/login/Login.tsx`):
//!
//! 1. Submitting the form opens `KunCaptchaModal`, which loads the `<cap-widget>`
//!    element pointed at `/api/auth/captcha/`.
//! 2. The widget fetches `POST /api/auth/captcha/challenge` and solves the PoW in
//!    WASM, then posts `{token, solutions}` to `POST /api/auth/captcha/redeem`.
//! 3. `redeem` stores a one-shot 32-hex code in Redis and returns it as `token`.
//! 4. `POST /api/auth/login` is called with `{name, password, captcha}`; the JWT
//!    session cookie `kun-galgame-patch-moe-token` (30 days) is set on success.
//!
//! Nothing here is account-gated: search, patch detail, resource lists and the
//! whole pan download chain work anonymously. A session only adds NSFW visibility
//! and personal state, so credentials stay optional.

use crate::captcha::{self, ChallengeResponse};
use crate::client::{Origin, TouchgalClient};
use crate::models::UserSession;
use crate::Error;

/// One-shot captcha code returned by `/api/auth/captcha/redeem`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RedeemResponse {
    pub success: bool,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub expires: Option<i64>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Requests a captcha challenge and solves its proof of work.
pub async fn solve_captcha(client: &TouchgalClient) -> crate::Result<String> {
    let challenge: ChallengeResponse = client
        .json(client.post(Origin::Site, "/api/auth/captcha/challenge"))
        .await?;
    let params = challenge.challenge;
    let solutions = captcha::solve(&challenge.token, params);
    if !captcha::verify(&challenge.token, params, &solutions) {
        return Err(Error::Captcha("local verification failed".into()));
    }
    let body = serde_json::json!({
        "token": challenge.token,
        "solutions": solutions,
    });
    let redeemed: RedeemResponse = client
        .json(
            client
                .post(Origin::Site, "/api/auth/captcha/redeem")
                .json(&body),
        )
        .await?;
    if !redeemed.success {
        return Err(Error::Captcha(
            redeemed.error.unwrap_or_else(|| "redeem rejected".into()),
        ));
    }
    redeemed
        .token
        .ok_or_else(|| Error::Captcha("redeem returned no token".into()))
}

/// Logs in and stores the session cookie on the client.
///
/// `name` may be a username or an e-mail address, matching `loginSchema`.
pub async fn login(
    client: &mut TouchgalClient,
    name: &str,
    password: &str,
) -> crate::Result<UserSession> {
    let captcha_code = solve_captcha(client).await?;
    let body = serde_json::json!({
        "name": name,
        "password": password,
        "captcha": captcha_code,
    });
    let session: UserSession = client
        .json(client.post(Origin::Site, "/api/auth/login").json(&body))
        .await?;
    if session.require_2fa {
        return Err(Error::Business(
            "该账号启用了两步验证, 请先在网页完成 2FA".into(),
        ));
    }
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_redeem_success_and_failure() {
        let ok: RedeemResponse =
            serde_json::from_str(r#"{"success":true,"token":"3b37348a98df7e499defa7e326e8481f","expires":1792682500038}"#)
                .unwrap();
        assert!(ok.success);
        assert_eq!(ok.token.unwrap().len(), 32);

        let bad: RedeemResponse =
            serde_json::from_str(r#"{"success":false,"error":"人机验证失败, 请重试"}"#).unwrap();
        assert!(!bad.success);
        assert_eq!(bad.error.unwrap(), "人机验证失败, 请重试");
    }
}
