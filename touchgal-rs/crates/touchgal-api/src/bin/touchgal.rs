//! `touchgal` — command line client for the reconstructed chain.
//!
//! ```text
//! touchgal search 千恋万花
//! touchgal suggest 千恋 Yuzusoft
//! touchgal patch a555829b
//! touchgal chain --keyword 千恋万花 --official --urls
//! touchgal pan 5yqBTE
//! touchgal download 5yqBTE --path /file.rar
//! ```

use clap::{Parser, Subcommand};
use touchgal_core::{auth, chain, pan, patch, resource, search, ClientConfig, TouchgalClient};

#[derive(Debug, Parser)]
#[command(name = "touchgal", version, about = "TouchGal chain CLI")]
struct Cli {
    /// Print compact JSON instead of pretty JSON.
    #[arg(long, global = true)]
    compact: bool,
    /// Optional session cookie for NSFW visibility.
    #[arg(long, global = true, env = "TOUCHGAL_SESSION_TOKEN")]
    session_token: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Search Galgame by keyword.
    Search {
        keyword: String,
        #[arg(long, default_value_t = 12)]
        limit: u32,
        #[arg(long, default_value_t = 1)]
        page: u32,
        #[arg(long)]
        platform: Option<String>,
        #[arg(long = "type")]
        kind: Option<String>,
        #[arg(long)]
        language: Option<String>,
        #[arg(long, default_value = "resource_update_time")]
        sort_field: String,
        #[arg(long, default_value = "desc")]
        sort_order: String,
    },
    /// Typeahead suggestions (tags + companies).
    Suggest { keywords: Vec<String> },
    /// Browse the galgame list with filters only.
    Browse {
        #[arg(long, default_value_t = 1)]
        page: u32,
        #[arg(long, default_value_t = 12)]
        limit: u32,
    },
    /// Patch detail by uniqueId (or a URL containing it).
    Patch { id: String },
    /// Resource list of a patch.
    Resources {
        patch_id: i64,
        #[arg(long)]
        official_only: bool,
    },
    /// Pan share metadata + file listing.
    Pan {
        key: String,
        #[arg(long)]
        path: Option<String>,
    },
    /// Presigned download URL for one pan file.
    Download {
        key: String,
        #[arg(long)]
        path: Option<String>,
    },
    /// Resolve the whole download chain.
    Chain {
        #[arg(long)]
        keyword: Option<String>,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        patch_id: Option<i64>,
        #[arg(long)]
        official: bool,
        #[arg(long)]
        urls: bool,
        #[arg(long)]
        count: bool,
    },
    /// Log in (captcha + JWT) and print the session.
    Login { name: String, password: String },
}

fn print_json<T: serde::Serialize>(value: &T, compact: bool) -> anyhow::Result<()> {
    let text = if compact {
        serde_json::to_string(value)?
    } else {
        serde_json::to_string_pretty(value)?
    };
    println!("{text}");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = ClientConfig {
        session_token: cli.session_token.clone(),
        ..Default::default()
    };
    let mut client = TouchgalClient::new(config)?;

    match cli.command {
        Command::Search {
            keyword,
            limit,
            page,
            platform,
            kind,
            language,
            sort_field,
            sort_order,
        } => {
            let (field, order) = match sort_field.as_str() {
                "created" => (search::SortField::Created, parse_order(&sort_order)),
                "rating" => (search::SortField::Rating, parse_order(&sort_order)),
                "view" => (search::SortField::View, parse_order(&sort_order)),
                "download" => (search::SortField::Download, parse_order(&sort_order)),
                "favorite" => (search::SortField::Favorite, parse_order(&sort_order)),
                _ => (
                    search::SortField::ResourceUpdateTime,
                    parse_order(&sort_order),
                ),
            };
            let mut query = search::SearchQuery::keyword(keyword)
                .with_limit(limit)
                .with_page(page)
                .with_sort(field, order);
            if let Some(platform) = platform {
                query = query.with_platform(platform);
            }
            if let Some(kind) = kind {
                query = query.with_type(kind);
            }
            if let Some(language) = language {
                query = query.with_language(language);
            }
            print_json(&search::search(&client, &query).await?, cli.compact)?;
        }
        Command::Suggest { keywords } => {
            print_json(&search::suggest(&client, &keywords).await?, cli.compact)?;
        }
        Command::Browse { page, limit } => {
            print_json(
                &search::browse(
                    &client,
                    &search::BrowseQuery {
                        page,
                        limit,
                        ..Default::default()
                    },
                )
                .await?,
                cli.compact,
            )?;
        }
        Command::Patch { id } => {
            let unique_id = patch::unique_id_from_url(&id)
                .ok_or_else(|| anyhow::anyhow!("no 8-character uniqueId in {id}"))?;
            print_json(&patch::get(&client, &unique_id).await?, cli.compact)?;
        }
        Command::Resources {
            patch_id,
            official_only,
        } => {
            let resources = resource::list(&client, patch_id).await?;
            if official_only {
                print_json(&resource::official_resources(&resources), cli.compact)?;
            } else {
                print_json(&resources, cli.compact)?;
            }
        }
        Command::Pan { key, path } => {
            let info = pan::share_info(&client, &key).await?;
            let listing = pan::list(&client, &key, path.as_deref()).await.ok();
            print_json(
                &serde_json::json!({ "info": info, "objects": listing.map(|l| l.objects) }),
                cli.compact,
            )?;
        }
        Command::Download { key, path } => {
            let url = pan::download_url(&client, &key, path.as_deref()).await?;
            print_json(&serde_json::json!({ "url": url }), cli.compact)?;
        }
        Command::Chain {
            keyword,
            id,
            patch_id,
            official,
            urls,
            count,
        } => {
            let mut options = if official {
                chain::ResolveOptions::official()
            } else {
                chain::ResolveOptions::default()
            };
            options.fetch_urls = urls;
            options.count_downloads = count;
            let result = if let Some(keyword) = keyword {
                chain::resolve_by_keyword(&client, &keyword, options).await?
            } else if let Some(id) = id {
                let unique_id = patch::unique_id_from_url(&id)
                    .ok_or_else(|| anyhow::anyhow!("no 8-character uniqueId in {id}"))?;
                chain::resolve_by_unique_id(&client, &unique_id, options).await?
            } else if let Some(patch_id) = patch_id {
                chain::resolve_download(&client, patch_id, options).await?
            } else {
                anyhow::bail!("one of --keyword, --id or --patch-id is required");
            };
            print_json(&result, cli.compact)?;
        }
        Command::Login { name, password } => {
            let session = auth::login(&mut client, &name, &password).await?;
            print_json(&session, cli.compact)?;
        }
    }
    Ok(())
}

fn parse_order(raw: &str) -> search::SortOrder {
    match raw {
        "asc" => search::SortOrder::Asc,
        _ => search::SortOrder::Desc,
    }
}
