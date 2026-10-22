//! Live integration tests against the real deployment.
//!
//! They are `#[ignore]` so `cargo test` stays offline and deterministic. Run with:
//!
//! ```text
//! cargo test -p touchgal-core --test live -- --ignored --nocapture
//! ```

use touchgal_core::{chain, pan, patch, resource, search, ClientConfig, TouchgalClient};

fn client() -> TouchgalClient {
    TouchgalClient::new(ClientConfig::default()).expect("client")
}

#[tokio::test]
#[ignore = "hits the live touchgal.ink deployment"]
async fn search_returns_the_expected_card() {
    let client = client();
    let results = search::search(&client, &search::SearchQuery::keyword("千恋万花"))
        .await
        .expect("search");
    assert!(results.total >= 1);
    let card = &results.galgames[0];
    assert_eq!(card.unique_id, "a555829b");
    assert_eq!(card.name, "千恋＊万花");
}

#[tokio::test]
#[ignore = "hits the live touchgal.ink deployment"]
async fn suggest_returns_tags_and_companies() {
    let client = client();
    let suggestions = search::suggest(&client, &["Yuzusoft".to_string()])
        .await
        .expect("suggest");
    assert!(!suggestions.is_empty());
    assert!(suggestions
        .iter()
        .any(|s| s.r#type == "company" && s.name == "Yuzusoft"));
}

#[tokio::test]
#[ignore = "hits the live touchgal.ink deployment"]
async fn patch_detail_and_official_resources() {
    let client = client();
    let detail = patch::get(&client, "a555829b").await.expect("patch");
    assert_eq!(detail.id, 637);
    let resources = resource::list(&client, detail.id).await.expect("resources");
    let official = resource::official_resources(&resources);
    assert_eq!(official.len(), 3);
    assert!(official.iter().any(|r| r
        .links
        .iter()
        .any(|l| l.content == "https://pan.touchgal.net/s/5yqBTE")));
}

#[tokio::test]
#[ignore = "hits the live pan deployment"]
async fn pan_share_listing_and_presigned_url() {
    let client = client();
    let info = pan::share_info(&client, "5yqBTE").await.expect("info");
    assert!(info.is_dir);
    assert!(!info.locked);
    let files = pan::list_files_recursive(&client, "5yqBTE", 2)
        .await
        .expect("files");
    assert_eq!(files.len(), 5);
    let url = pan::download_url(&client, "5yqBTE", Some(&files[0].full_path()))
        .await
        .expect("presigned");
    assert!(url.starts_with("https://"));
    assert!(url.contains("X-Amz-Signature"));
}

#[tokio::test]
#[ignore = "hits the live deployments"]
async fn full_chain_resolves_official_downloads() {
    let client = client();
    let full = chain::resolve_download(&client, 637, chain::ResolveOptions::official().with_urls())
        .await
        .expect("chain");
    assert_eq!(full.unique_id, "a555829b");
    assert_eq!(full.official_resource_count, 3);
    let with_urls = full
        .resources
        .iter()
        .flat_map(|r| r.links.iter())
        .flat_map(|l| l.files.iter())
        .filter(|f| f.download_url.is_some())
        .count();
    assert!(with_urls >= 5, "expected presigned urls, got {with_urls}");
}
