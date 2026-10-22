# TouchGal API kernel (Rust)

A Rust re-implementation of the network behaviour behind
[touchgal.ink](https://www.touchgal.ink) — the open-source
[KunMoe/kun-touchgal-next](https://github.com/KunMoe/kun-touchgal-next) site — for
the **search → official download → resource-pan jump → file download** chain.

Everything here was reconstructed from the shipped source plus live observation
(see [`docs/reverse-engineering.md`](docs/reverse-engineering.md)); the crate is a
drop-in kernel that other applications can link against, wrap in the bundled HTTP
server, or drive from the bundled CLI.

```text
search ──► patch detail ──► resource list ──► /redirect?url=… ──► pan share ──► presigned S3 URL
 (POST)      (GET)             (GET)             (5 s interstitial)   (Cloudreve v3)
```

## Layout

| path | what it is |
|---|---|
| [`touchgal-rs/crates/touchgal-core`](touchgal-rs/crates/touchgal-core) | the kernel: HTTP transport, models, search, patch, resources, pan, chain, captcha/login |
| [`touchgal-rs/crates/touchgal-api`](touchgal-rs/crates/touchgal-api) | `touchgal-api` HTTP server + `touchgal` CLI |
| [`docs/reverse-engineering.md`](docs/reverse-engineering.md) | the observed endpoints, payload shapes and anti-bot constraints |
| [`docs/rea-evidence.md`](docs/rea-evidence.md) | the REA evidence ids and what each one does (and does not) establish |
| [`shots/`](shots) | headless-browser screenshots of each hop |
| [`research/`](research) | throwaway CDP probe scripts used during the investigation |

## Quick start

```bash
cd touchgal-rs
cargo test                       # 18 unit tests, no network
cargo run -p touchgal-core --example live_check    # live end-to-end against touchgal.ink
cargo run --release -p touchgal-api --bin touchgal-api -- --bind 127.0.0.1:8787
```

### CLI

```bash
# search
touchgal search 千恋万花 --limit 3
touchgal suggest 千恋 Yuzusoft

# patch detail / resources
touchgal patch a555829b
touchgal resources 637 --official-only

# pan + presigned URL
touchgal pan 5yqBTE
touchgal download wpwtv

# the whole chain in one call
touchgal chain --keyword 千恋万花 --official --urls
```

`touchgal chain --keyword 千恋万花 --official` prints, for the official
resources only:

```json
{
  "patch_id": 637,
  "unique_id": "a555829b",
  "name": "千恋＊万花",
  "official_resource_count": 3,
  "resources": [
    {
      "id": 9094,
      "name": "STEAM官方中文版",
      "links": [
        {
          "storage": "touchgal",
          "size": "8.4GB",
          "content": "https://pan.touchgal.net/s/5yqBTE",
          "target": {
            "kind": "pan_share",
            "key": "5yqBTE",
            "url": "https://pan.touchgal.net/s/5yqBTE",
            "interstitial_url": "https://www.touchgal.ink/redirect?url=https%3A%2F%2Fpan.touchgal.net%2Fs%2F5yqBTE",
            "delay_seconds": 5
          },
          "files": [
            { "name": "(無修正ゲーム) Senren Banka Steam Edition.part1.rar", "path": "/…part1.rar", "size": 2097152000 }
          ]
        }
      ]
    }
  ]
}
```

### HTTP API

```bash
touchgal-api --bind 127.0.0.1:8787
```

| endpoint | purpose |
|---|---|
| `GET /healthz` | liveness |
| `GET /v1/search?q=千恋万花&limit=12&page=1&platform=apk&sort_field=rating` | `POST /api/search` |
| `GET /v1/suggest?q=千恋,Yuzusoft` | `POST /api/search/tag` |
| `GET /v1/browse?page=1&limit=24` | `GET /api/galgame` |
| `GET /v1/patch?id=a555829b` | `GET /api/patch` |
| `GET /v1/resources?patch_id=637&official_only=true` | `GET /api/patch/resource` |
| `GET /v1/pan/info?key=5yqBTE` | Cloudreve share metadata |
| `GET /v1/pan/list?key=5yqBTE&recursive=true` | share listing / recursive file walk |
| `GET /v1/pan/download?key=5yqBTE&path=/file.rar` | presigned URL |
| `GET /v1/chain?patch_id=637&official_only=true&urls=true` | full chain (also accepts `id=` or `q=`) |
| `GET /v1/login?name=…&password=…` | optional; captcha solved locally |

Success responses are the upstream document itself (same field names the site
uses). Failures keep the site's own semantics:

| case | HTTP | body |
|---|---|---|
| business error, e.g. unknown `uniqueId` | 200 | `{"ok":false,"error":"touchgal business error: 未找到对应 Galgame","kind":"business"}` |
| invalid argument | 400 | `kind:"invalid_argument"` |
| Cloudflare / transport / decode | 502 | `kind:"cloudflare" / "transport" / "decode"` |

### Library

```rust
use touchgal_core::{chain, search, ClientConfig, TouchgalClient};

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let client = TouchgalClient::new(ClientConfig::default())?;

let hits = search::search(&client, &search::SearchQuery::keyword("千恋万花")).await?;
let patch_id = hits.galgames[0].id;

let full = chain::resolve_download(
    &client,
    patch_id,
    chain::ResolveOptions::official().with_urls(),
).await?;
for resource in &full.resources {
    for link in &resource.links {
        for file in &link.files {
            println!("{} -> {:?}", file.name, file.download_url);
        }
    }
}
# Ok(()) }
```

## What is reproduced, and how it was verified

* **Search** — the wire payload is not a plain keyword: the page sends
  `queryString` as a JSON-encoded *suggestion array*
  (`[{type:"keyword"|"tag"|"company", mode:"include"|"exclude", id?, name}]`)
  together with `searchOption`, filters, paging and sort. `SearchQuery::to_payload`
  reproduces a body captured from the browser byte-for-byte (unit-tested), and the
  live call returns the same `{galgames, total}` document.
* **Official download** — `GET /api/patch/resource?patchId=` returns every public
  resource with its `links[]`; `storage` is `touchgal` (official pan), `s3`
  (object storage) or `user` (community mirror). Only the first is "official".
* **Resource-pan jump** — the UI never links out directly: every http link goes
  through `/redirect?url=…`, which shows a `delaySeconds` (5 s) countdown and
  rejects non-http schemes and blocked URLs. `resolve_link` reproduces that
  decision, including the `excludedDomains` bypass.
* **Download** — `pan.touchgal.net` is a Cloudreve v3 share host. The chain is
  `GET /api/v3/share/info/{key}` → `GET /api/v3/share/list/{key}%2F` (folder
  shares) → `PUT /api/v3/share/download/{key}?path=…`, which answers with a
  presigned `touchgaldownload.xyz` URL (verified: `HEAD` 200, ranged `GET` 206,
  ZIP magic `PK\x03\x04`).
* **Anti-bot** — plain curl/reqwest requests to `www.touchgal.ink` get Cloudflare's
  "Attention Required!" 403 even with a Chrome User-Agent; a TLS/HTTP2 fingerprint
  that matches Chrome passes. The client therefore uses `wreq`'s Chrome emulation,
  plus the app's own CSRF header (`x-requested-with: kun-fetch`) and a same-site
  `origin`/`referer` on writes.
* **Login (optional)** — the site gates login behind a Cap proof-of-work captcha.
  `captcha.rs` ports `capjs-core`'s FNV-1a + xorshift32 PRNG and the
  `sha256(salt + nonce)` prefix search, verified against a nonce produced by the
  real browser solver. Nothing else in the chain needs an account.

## Notes / limits

* Read-only: this kernel never publishes, uploads, likes or comments.
* Community mirrors (Quark/Baidu/139/…) are returned as targets only — their
  download flows belong to those services, not to TouchGal.
* `pan` folder shares expose no archive endpoint on this deployment
  (`/share/archive` answers 40007), so multi-part releases are enumerated and
  downloaded file by file.
* `cf_clearance` is not required for TLS-emulated requests, but
  `ClientConfig::cf_clearance` / `TOUCHGAL_SESSION_TOKEN` let an operator reuse a
  browser session for NSFW content.

## Verification

Offline (`cargo test`) — 18 unit tests pinning the wire formats against values
captured from the live site: the search payload byte-for-byte, the captcha PRNG
chain (including nonce `55266` produced by the browser's WASM solver), the
`/redirect` decision table, pan path encoding, and real JSON fixtures.

Online (`cargo test -p touchgal-core --test live -- --ignored`) — 5 tests that
hit the deployment:

```text
test search_returns_the_expected_card ... ok
test suggest_returns_tags_and_companies ... ok
test patch_detail_and_official_resources ... ok
test pan_share_listing_and_presigned_url ... ok
test full_chain_resolves_official_downloads ... ok
```

Manual end-to-end (all against production, no account):

```text
touchgal search 千恋万花        -> 1 hit, uniqueId a555829b
touchgal resources 637         -> 57 resources, 3 official
touchgal pan 5yqBTE            -> is_dir=true, 5 part files
touchgal download wpwtv        -> presigned 4.0 GB ZIP URL (HEAD 200, GET 206, PK\x03\x04)
touchgal chain --keyword 千恋万花 --official --urls -> 3 resources / 7 presigned URLs
touchgal login <username> …    -> session payload (captcha solved locally in ~3 s)
```
