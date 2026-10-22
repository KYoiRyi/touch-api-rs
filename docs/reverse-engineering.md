# Reverse-engineering notes

Target: <https://www.touchgal.ink> — the deployment of
[`KunMoe/kun-touchgal-next`](https://github.com/KunMoe/kun-touchgal-next) v1.0.7
(Next.js 16 App Router + Prisma 7/PostgreSQL + Meilisearch).

Method: read the shipped source in [`repo/`](../repo) for the *shape* of each
request, then confirm every payload against the live deployment with a headless
Chrome driven over CDP ([`research/`](../research), screenshots in
[`shots/`](../shots)). Nothing was taken on faith from the source: several
details (Cloudflare behaviour, the pan protocol, the captcha) only exist at
runtime.

## 1. Anti-bot layer

```text
$ curl -X POST https://www.touchgal.ink/api/search -H 'user-agent: Chrome/155 …'
403  <!DOCTYPE html> … "Attention Required! | Cloudflare"  (cf-ray …-HKG)
```

* Plain curl/Node `fetch` with a Chrome User-Agent is **blocked** on `POST` to
  `/api/*`; `GET` on some endpoints happens to pass.
* A headless Chrome that solved the challenge gets `cf_clearance`; reusing that
  cookie *plus the exact UA* from Node makes the POST succeed, i.e. the block is
  a TLS/HTTP2 fingerprint check plus a cookie check, not a JS challenge on the API.
* Consequence for the kernel: use a client with a real Chrome TLS/HTTP2
  fingerprint. `wreq` + `wreq-util::Emulation::Chrome136` passes with no cookies
  at all (verified: `POST /api/search` → 200, `POST /api/auth/captcha/challenge` → 200).
  `reqwest`/rustls and `impit` (no usable 0.1 API at the time) did not.

The app's own CSRF middleware (`middleware/_csrf.ts`) additionally requires

```text
x-requested-with: kun-fetch
origin:  https://www.touchgal.ink      (or referer)
```

on every `POST/PUT/PATCH/DELETE`, otherwise the handler answers `"非法请求来源"`.

## 2. Search

The search page does **not** send a keyword. `components/search/Container.tsx`
builds a suggestion array and JSON-encodes it into `queryString`
(`validations/search.ts`):

```json
POST /api/search
{
  "queryString": "[{\"type\":\"keyword\",\"mode\":\"include\",\"name\":\"千恋万花\"}]",
  "limit": 12,
  "searchOption": { "searchInIntroduction": false, "searchInAlias": true, "searchInTag": false },
  "page": 1,
  "selectedType": "all",
  "selectedLanguage": "all",
  "selectedPlatform": "all",
  "sortField": "resource_update_time",
  "sortOrder": "desc",
  "selectedYears": ["all"],
  "selectedMonths": ["all"],
  "minRatingCount": 0
}
```

```json
200 {"galgames":[{"id":637,"uniqueId":"a555829b","name":"千恋＊万花","banner":"…",
"view":1852348,"download":668695,"type":[…],"language":[…],"platform":[…],
"created":"2023-06-08T19:03:32.000Z","_count":{"favorite_folder":5534,
"resource":57,"comment":375},"averageRating":8.5}],"total":1}
```

Suggestion semantics (`app/api/search/route.ts`):

| field | meaning |
|---|---|
| `type` | `keyword` (full-text), `tag` (tag id or name), `company` (company id or name) |
| `mode` | `include` / `exclude`; omitted means `include` (legacy payloads) |
| `id` | optional; when present the id is used directly, otherwise names are resolved server-side |

Other observations, each checked live:

* `minRatingCount` only applies when `sortField = "rating"` on `/api/search`
  (always applies on `/api/galgame`).
* `selectedYears` / `selectedMonths` are string arrays; `"all"`, `"future"`,
  `"unknown"` are special values, otherwise a 4-digit year and a 1-2 digit month.
  `selectedYears:["2016"]` → 178 hits, `["2016","2017"]` → 339.
* `limit` is capped at 24, `queryString` at 1007 characters, and a bare VNDB id
  (`v19073`) is matched exactly instead of full-text.
* Typeahead: `POST /api/search/tag {"query":["千恋","Yuzusoft"]}` → tag/company
  chips `{id,type,mode,name}`, sorted by usage count, max 100.
* Browse list: `GET /api/galgame` with the same filters, `yearString` /
  `monthString` as JSON-encoded strings.

## 3. Patch detail and resource list

```text
GET /api/patch?uniqueId=a555829b     → Patch {id, uniqueId, name, introduction,
                                            alias[], tags[], contentLimit,
                                            ratingSummary, _count{…}}
GET /api/patch/resource?patchId=637  → PatchResource[] with links[]
```

`uniqueId` is exactly 8 alphanumeric characters and is the page slug
(`/{uniqueId}`). `status` visibility: `0` public, `2/3` pending (author + role ≥ 3
only), `1` hidden.

Each resource carries `links[]`:

```json
{"id":8675,"storage":"touchgal","size":"8.4GB","code":"","password":"","hash":"",
 "content":"https://pan.touchgal.net/s/5yqBTE","sortOrder":0,"download":33138}
```

| `storage` | meaning | where `content` points |
|---|---|---|
| `touchgal` | official pan (only super-admins can publish) | `https://pan.touchgal.net/s/<key>` |
| `s3` | object storage upload | direct `…/patch/<id>/resource/<hash>/<file>` URL, with a BLAKE3 `hash` |
| `user` | community mirror | Quark / Baidu / 139 / Xunlei link, may carry `code`/password |

Clicking a link first fires the counter (anonymous is fine, deduplicated per
identity+link for a cache window):

```text
PUT /api/patch/resource/download {"patchId":637,"resourceId":9094,"linkId":8675} → {}
```

## 4. The `/redirect` hop

`components/kun/external-link/ExternalLink.tsx` + `utils/safeUrl.ts` +
`config/redirect.json`:

* Only `http(s)` links are bounced through `/redirect?url=<encodeURIComponent>`.
* `magnet:`, `ed2k:`, `thunder:`, `mailto:`, `irc(s):`, `xmpp:` are allowed but not bounced.
* Hosts matching `excludedDomains` (`touchgal.ink`, `kungal.com`) bypass it.
* `enableRedirect:false` (per user) bypasses it.
* The page itself refuses non-http schemes and shows "检测到不安全或无效的跳转链接, 已阻止跳转".
* The button stays disabled for `delaySeconds` (default 5 s).

Captured live: clicking the official 8.4 GB link navigates to
`https://www.touchgal.ink/redirect?url=https%3A%2F%2Fpan.touchgal.net%2Fs%2F5yqBTE`
(see [`shots/3-redirect-interstitial.png`](../shots/3-redirect-interstitial.png)).

## 5. `pan.touchgal.net` (Cloudreve v3)

The pan is a Cloudreve deployment. Endpoints were extracted from
`/static/js/main.*.chunk.js` and then exercised directly:

| request | response |
|---|---|
| `GET /api/v3/share/info/{key}` | `{code:0,data:{key,locked,is_dir,downloads,views,creator,source}}` |
| `GET /api/v3/share/list/{key}%2F[?path=…]` | `{code:0,data:{objects:[{id,name,path,size,type,date}]}}` |
| `PUT /api/v3/share/download/{key}[?path=…]` | `{code:0,data:"<presigned https://…touchgaldownload.xyz/…>"}` |
| `POST /api/v3/share/archive/{key}` | **40007** — archive download is disabled here |

Notes:

* Single-file shares (`is_dir:false`) download with no `path`; folder shares need
  the encoded file path.
* The presigned URL is a Backblaze-B2 style S3 signature valid for 1 hour.
  Verified end-to-end: `HEAD` → 200 `application/zip` `content-length: 4044897775`,
  ranged `GET` → 206 with `50 4b 03 04` (ZIP).
* The pan endpoints are **not** behind the Cloudflare bot rule — plain requests work.
* No account is needed at any point.

## 6. Login (optional)

```text
POST /api/auth/captcha/challenge   → {"challenge":{"c":50,"s":32,"d":4},"token":"<JWT>"}
POST /api/auth/captcha/redeem      {"token","solutions":[nonce…]} → {"success":true,"token":"<32 hex>"}
POST /api/auth/login               {"name","password","captcha"} → UserState + Set-Cookie kun-galgame-patch-moe-token (30 d)
```

The captcha is Cap (`capjs-core`). The challenge is an HMAC JWT whose payload is
`{n,c,s,d,exp,iat,sk:"kun-captcha"}`; each of the `c` sub-challenges derives

```text
tokenFnv  = fnv1a(token)
saltSeed  = fnv1a_resume(tokenFnv, str(i+1))
targetSeed= fnv1a_resume(saltSeed, "d")
salt      = prng_from_hash(saltSeed, s)      # xorshift32 → hex
target    = prng_from_hash(targetSeed, d)
solution  = smallest nonce with sha256(salt + nonce) hex starting with target
```

`captcha.rs` ports that exactly; the unit test reproduces nonce `55266` for a
nonce produced by the real browser WASM solver on the same token, and a full
50-challenge solve + `POST /api/auth/login` with the test account returns
a `UserState` payload (`{"uid":…,"name":…,"role":…}`) in ~3 s (release build).

## 7. Everything anonymous

Search, patch detail, resource lists, the redirect decision and the whole pan
download chain work **without** an account; a session only adds NSFW visibility
(`contentLimit`) and personal state. The kernel therefore treats credentials as
optional.

## Screenshots

| hop | file |
|---|---|
| patch detail | [`shots/0-patch-detail.png`](../shots/0-patch-detail.png) |
| search results | [`shots/1-search-results.png`](../shots/1-search-results.png) |
| official resource links | [`shots/2-official-resource-links.png`](../shots/2-official-resource-links.png) |
| `/redirect` interstitial | [`shots/3-redirect-interstitial.png`](../shots/3-redirect-interstitial.png) |
| pan share listing | [`shots/4-pan-share-listing.png`](../shots/4-pan-share-listing.png) |
| advanced filters | [`shots/5-advanced-filters.png`](../shots/5-advanced-filters.png) |
