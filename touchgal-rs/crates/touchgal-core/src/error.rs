use thiserror::Error;

/// Errors surfaced by the TouchGal API core.
///
/// TouchGal route handlers answer business failures with a bare JSON string
/// (e.g. `"未找到该资源"`) and HTTP 200, so [`Error::Business`] carries that
/// payload verbatim instead of inventing a status code.
#[derive(Debug, Error)]
pub enum Error {
    #[error("transport error: {0}")]
    Transport(#[from] wreq::Error),

    #[error("unexpected response body: {0}")]
    Decode(String),

    #[error("touchgal business error: {0}")]
    Business(String),

    #[error("cloudflare challenge blocked the request (status {status})")]
    CloudflareBlocked { status: u16 },

    #[error("unexpected http status {status} for {url}")]
    Status { status: u16, url: String },

    #[error("captcha could not be solved: {0}")]
    Captcha(String),

    #[error("not logged in")]
    Unauthorized,

    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn is_business(&self) -> bool {
        matches!(self, Error::Business(_))
    }
}
