//! End-to-end live check against the real deployment.
//!
//! Run with: `cargo run -p touchgal-core --example live_check`
//!
//! Exercises the exact chain the site uses, anonymously:
//! search -> patch detail -> resource list -> official pan link -> presigned URL.

use touchgal_core::{chain, pan, patch, resource, search, ClientConfig, TouchgalClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = TouchgalClient::new(ClientConfig::default())?;

    // 1. search
    let results = search::search(&client, &search::SearchQuery::keyword("千恋万花")).await?;
    println!(
        "[search] total={} first={:?}",
        results.total,
        results.galgames.first().map(|g| (&g.name, &g.unique_id))
    );
    let card = results.galgames.first().ok_or("no result")?;
    let unique_id = card.unique_id.clone();
    let patch_id = card.id;

    // 2. typeahead
    let suggestions =
        search::suggest(&client, &["千恋".to_string(), "Yuzusoft".to_string()]).await?;
    println!(
        "[suggest] {} -> {:?}",
        suggestions.len(),
        suggestions
            .iter()
            .take(3)
            .map(|s| (&s.r#type, &s.name))
            .collect::<Vec<_>>()
    );

    // 3. patch detail
    let detail = patch::get(&client, &unique_id).await?;
    println!(
        "[patch] {} ({}) content_limit={} resources={}",
        detail.name, detail.unique_id, detail.content_limit, detail.count.resource
    );

    // 4. resource list
    let resources = resource::list(&client, patch_id).await?;
    let official = resource::official_resources(&resources);
    println!(
        "[resource] {} resources, {} official",
        resources.len(),
        official.len()
    );

    // 5. official pan chain
    for res in official.iter().take(2) {
        for link in &res.links {
            if link.storage != resource::STORAGE_TOUCHGAL {
                continue;
            }
            let key = touchgal_core::util::pan_share_key(&link.content).ok_or("no key")?;
            let info = pan::share_info(&client, &key).await?;
            println!(
                "[pan] {} key={} is_dir={} downloads={}",
                link.content, key, info.is_dir, info.downloads
            );
            if info.is_dir {
                let files = pan::list_files_recursive(&client, &key, 3).await?;
                println!("[pan] {} files", files.len());
                if let Some(first) = files.first() {
                    let url = pan::download_url(&client, &key, Some(&first.full_path())).await?;
                    println!(
                        "[pan] presigned: {}…",
                        url.chars().take(110).collect::<String>()
                    );
                }
            } else {
                let url = pan::download_url(&client, &key, None).await?;
                println!(
                    "[pan] presigned single-file: {}…",
                    url.chars().take(110).collect::<String>()
                );
            }
        }
    }

    // 6. redirect decision for a community mirror
    let community = resources
        .iter()
        .flat_map(|r| r.links.iter())
        .find(|l| l.storage == resource::STORAGE_USER);
    if let Some(link) = community {
        let target = resource::resolve_link(
            &link.content,
            true,
            &["touchgal.ink".into(), "kungal.com".into()],
            5,
        );
        println!("[redirect] {} -> {:?}", link.content, target);
    }

    // 7. whole chain in one call
    let full = chain::resolve_download(
        &client,
        patch_id,
        chain::ResolveOptions::official().with_urls(),
    )
    .await?;
    println!(
        "[chain] {} resources, {} official, urls={}",
        full.resources.len(),
        full.official_resource_count,
        full.resources
            .iter()
            .flat_map(|r| r.links.iter())
            .flat_map(|l| l.files.iter())
            .filter(|f| f.download_url.is_some())
            .count()
    );

    Ok(())
}
