//! # touchgal-core
//!
//! A Rust re-implementation of the network behaviour of
//! [TouchGal](https://www.touchgal.ink) (`KunMoe/kun-touchgal-next`), the
//! Galgame community site, for use as an embeddable API kernel.
//!
//! ## Scope
//!
//! Only the anonymous, read-only download path is modelled, because that is what
//! the site serves without an account:
//!
//! 1. [`search`] — `POST /api/search` (suggestion chips, filters, sorting) and
//!    `POST /api/search/tag` (typeahead), plus `GET /api/galgame` browsing.
//! 2. [`patch`] — `GET /api/patch?uniqueId=…` detail payload.
//! 3. [`resource`] — `GET /api/patch/resource?patchId=…`, the per-link
//!    `storage` taxonomy (`touchgal` / `s3` / `user`), the `/redirect`
//!    decision, and the anonymous `PUT /api/patch/resource/download` counter.
//! 4. [`pan`] — `pan.touchgal.net`, a Cloudreve v3 share host: share metadata,
//!    directory listings and presigned S3 download URLs.
//! 5. [`chain`] — the whole chain in one call.
//!
//! [`auth`] additionally reproduces the login path (Cap proof-of-work captcha +
//! JWT session cookie) so a caller can opt into NSFW visibility; nothing else in
//! this crate requires it.
//!
//! ## Transport notes
//!
//! `www.touchgal.ink` is behind Cloudflare bot management. Requests that do not
//! look like a real Chrome get an HTML challenge instead of JSON, so
//! [`client::TouchgalClient`] uses `wreq`'s Chrome emulation profile and adds the
//! `x-requested-with: kun-fetch` CSRF header the app requires on writes.

pub mod auth;
pub mod captcha;
pub mod chain;
pub mod client;
pub mod error;
pub mod models;
pub mod pan;
pub mod patch;
pub mod resource;
pub mod search;
pub mod util;

pub use client::{ClientConfig, Origin, TouchgalClient};
pub use error::{Error, Result};

/// Convenience constructor used by the CLI and the HTTP server.
pub fn client() -> Result<TouchgalClient> {
    TouchgalClient::new(ClientConfig::default())
}
