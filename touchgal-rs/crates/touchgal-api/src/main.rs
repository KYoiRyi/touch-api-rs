//! `touchgal-api` — HTTP server exposing the reconstructed TouchGal chain.
//!
//! ```text
//! touchgal-api --bind 127.0.0.1:8787
//! ```
//!
//! Endpoints (all GET, JSON):
//! * `/v1/search?q=千恋万花&limit=12&page=1&platform=apk`
//! * `/v1/suggest?q=千恋,Yuzusoft`
//! * `/v1/browse?page=1&limit=24&sort_field=rating`
//! * `/v1/patch?id=a555829b`
//! * `/v1/resources?patch_id=637&official_only=true`
//! * `/v1/pan/info?key=5yqBTE`
//! * `/v1/pan/list?key=5yqBTE&recursive=true`
//! * `/v1/pan/download?key=5yqBTE&path=/file.rar`
//! * `/v1/chain?patch_id=637&official_only=true&urls=true`
//! * `/v1/login?name=…&password=…` (optional; nothing else needs an account)

use clap::Parser;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "touchgal-api",
    version,
    about = "HTTP API kernel for the TouchGal (touchgal.ink) search / official-pan download chain"
)]
struct Args {
    /// Address to bind.
    #[arg(long, env = "TOUCHGAL_BIND", default_value = "127.0.0.1:8787")]
    bind: String,
    /// Site base URL (mirror domain support).
    #[arg(
        long,
        env = "TOUCHGAL_SITE_BASE",
        default_value = "https://www.touchgal.ink"
    )]
    site_base: String,
    /// Pan base URL.
    #[arg(
        long,
        env = "TOUCHGAL_PAN_BASE",
        default_value = "https://pan.touchgal.net"
    )]
    pan_base: String,
    /// Optional session cookie (`kun-galgame-patch-moe-token`) for NSFW content.
    #[arg(long, env = "TOUCHGAL_SESSION_TOKEN")]
    session_token: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let config = touchgal_core::ClientConfig {
        site_base: args.site_base.clone(),
        pan_base: args.pan_base.clone(),
        session_token: args.session_token.clone(),
        ..Default::default()
    };

    let client = touchgal_core::TouchgalClient::new(config)?;
    let app = touchgal_api::router(client);

    let addr: SocketAddr = args.bind.parse()?;
    tracing::info!("touchgal-api listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
