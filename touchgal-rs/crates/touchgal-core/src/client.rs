//! HTTP transport for the TouchGal site and its resource pan.
//!
//! Observed constraints (verified against the live deployment on 2026-10-22):
//!
//! * `www.touchgal.ink` sits behind Cloudflare bot management. Plain
//!   `reqwest`/curl requests with a Chrome User-Agent still get the
//!   "Attention Required!" 403 page, while a TLS/HTTP2 fingerprint that matches a
//!   real Chrome passes. We therefore drive every request through `wreq`'s
//!   Chrome emulation profile.
//! * State-changing endpoints additionally require the CSRF header
//!   `x-requested-with: kun-fetch` plus a same-site `origin`/`referer`
//!   (`middleware/_csrf.ts`), otherwise the app answers `"非法请求来源"`.
//! * `pan.touchgal.net` is a Cloudreve v3 deployment whose share endpoints are
//!   anonymous and not Cloudflare-gated; only the site needs emulation.

use std::time::Duration;
use wreq::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, ORIGIN, REFERER, USER_AGENT,
};
use wreq::Client;
use wreq_util::Emulation;

use crate::Error;

pub const SITE_BASE: &str = "https://www.touchgal.ink";
pub const PAN_BASE: &str = "https://pan.touchgal.net";
/// Header the site's CSRF middleware requires on POST/PUT/PATCH/DELETE.
pub const CSRF_HEADER: &str = "x-requested-with";
pub const CSRF_VALUE: &str = "kun-fetch";

/// Which upstream host a request targets; decides the CSRF headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Site,
    Pan,
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Site origin, so a mirror domain can be substituted.
    pub site_base: String,
    pub pan_base: String,
    pub timeout: Duration,
    /// Optional `cf_clearance` cookie. Not required for TLS-emulated requests,
    /// but lets an operator reuse a browser session (needed for logged-in calls).
    pub cf_clearance: Option<String>,
    /// Session cookie for authenticated endpoints
    /// (`kun-galgame-patch-moe-token`).
    pub session_token: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            site_base: SITE_BASE.to_string(),
            pan_base: PAN_BASE.to_string(),
            timeout: Duration::from_secs(30),
            cf_clearance: None,
            session_token: None,
        }
    }
}

/// Thin wrapper that adds the observed browser/CSRF envelope to every call.
#[derive(Clone)]
pub struct TouchgalClient {
    http: Client,
    config: ClientConfig,
}

impl std::fmt::Debug for TouchgalClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TouchgalClient")
            .field("site_base", &self.config.site_base)
            .field("pan_base", &self.config.pan_base)
            .finish()
    }
}

impl TouchgalClient {
    pub fn new(config: ClientConfig) -> crate::Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
        );
        // The emulation profile sets a Chrome UA; keep an explicit one so the
        // value stays stable when the profile is upgraded.
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36",
            ),
        );
        let http = Client::builder()
            .emulation(Emulation::Chrome136)
            .cookie_store(true)
            .timeout(config.timeout)
            .default_headers(headers)
            .build()?;
        Ok(Self { http, config })
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub fn set_session_token(&mut self, token: Option<String>) {
        self.config.session_token = token;
    }

    pub fn http(&self) -> &Client {
        &self.http
    }

    fn base(&self, origin: Origin) -> &str {
        match origin {
            Origin::Site => &self.config.site_base,
            Origin::Pan => &self.config.pan_base,
        }
    }

    /// Builds a request with the CSRF envelope the site expects.
    pub fn request(
        &self,
        method: wreq::Method,
        origin: Origin,
        path: &str,
    ) -> wreq::RequestBuilder {
        let base = self.base(origin);
        let url = if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else {
            format!("{base}{path}")
        };
        let mut builder = self.http.request(method.clone(), url);
        if method != wreq::Method::GET && method != wreq::Method::HEAD {
            builder = builder
                .header(CSRF_HEADER, CSRF_VALUE)
                .header(ORIGIN, base)
                .header(REFERER, format!("{base}/"));
            if origin == Origin::Pan {
                // Cloudreve itself does not require it; harmless and keeps a
                // consistent browser-like envelope.
                builder = builder.header(CONTENT_TYPE, "application/json");
            }
        }
        if let Some(cookie) = &self.config.cf_clearance {
            builder = builder.header(wreq::header::COOKIE, format!("cf_clearance={cookie}"));
        }
        if origin == Origin::Site {
            if let Some(token) = &self.config.session_token {
                builder = builder.header(
                    wreq::header::COOKIE,
                    format!("kun-galgame-patch-moe-token={token}"),
                );
            }
        }
        builder
    }

    pub fn get(&self, origin: Origin, path: &str) -> wreq::RequestBuilder {
        self.request(wreq::Method::GET, origin, path)
    }

    pub fn post(&self, origin: Origin, path: &str) -> wreq::RequestBuilder {
        self.request(wreq::Method::POST, origin, path)
    }

    pub fn put(&self, origin: Origin, path: &str) -> wreq::RequestBuilder {
        self.request(wreq::Method::PUT, origin, path)
    }

    /// Reads a JSON body, translating the site's bare-string error payloads into
    /// [`Error::Business`].
    pub async fn json<T: serde::de::DeserializeOwned>(
        &self,
        builder: wreq::RequestBuilder,
    ) -> crate::Result<T> {
        let response = builder.send().await?;
        let status = response.status();
        let text = response.text().await?;
        decode_json(&text, status.as_u16())
    }

    /// Reads a JSON body from a Cloudreve-style envelope (`{code, data, msg}`).
    pub async fn json_envelope<T: serde::de::DeserializeOwned>(
        &self,
        builder: wreq::RequestBuilder,
    ) -> crate::Result<T> {
        let response = builder.send().await?;
        let status = response.status();
        let text = response.text().await?;
        parse_envelope(&text, status.as_u16())
    }
}

pub fn decode_json<T: serde::de::DeserializeOwned>(text: &str, status: u16) -> crate::Result<T> {
    if status == 403 && text.contains("Just a moment") {
        return Err(Error::CloudflareBlocked { status });
    }
    if status == 403 && text.contains("Attention Required") {
        return Err(Error::CloudflareBlocked { status });
    }
    let trimmed = text.trim_start();
    if trimmed.starts_with('"') {
        // TouchGal returns business errors as a bare JSON string.
        if let Ok(message) = serde_json::from_str::<String>(text) {
            return Err(Error::Business(message));
        }
    }
    serde_json::from_str::<T>(text).map_err(|e| {
        if status >= 400 {
            Error::Status {
                status,
                url: String::new(),
            }
        } else {
            Error::Decode(format!(
                "{e}: {}",
                text.chars().take(200).collect::<String>()
            ))
        }
    })
}

#[derive(Debug, serde::Deserialize)]
struct Envelope<T> {
    code: i64,
    data: Option<T>,
    #[serde(default)]
    msg: String,
}

pub fn parse_envelope<T: serde::de::DeserializeOwned>(text: &str, status: u16) -> crate::Result<T> {
    if status == 403 && text.contains("Just a moment") {
        return Err(Error::CloudflareBlocked { status });
    }
    let envelope: Envelope<T> = serde_json::from_str(text).map_err(|e| {
        Error::Decode(format!(
            "{e}: {}",
            text.chars().take(200).collect::<String>()
        ))
    })?;
    if envelope.code != 0 {
        return Err(Error::Business(if envelope.msg.is_empty() {
            format!("cloudreve error code {}", envelope.code)
        } else {
            envelope.msg
        }));
    }
    envelope
        .data
        .ok_or_else(|| Error::Decode("cloudreve envelope had no data".into()))
}
