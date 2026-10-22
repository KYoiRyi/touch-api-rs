# REA evidence log

Observations taken with the REA MCP browser provider against a headless Chrome
(155.0.8059.39) attached over CDP at `127.0.0.1:9222`. These corroborate the
findings in [`reverse-engineering.md`](reverse-engineering.md); the screenshots in
[`../shots/`](../shots) were captured from the same session.

| evidence id | operation | what it establishes |
|---|---|---|
| `ev_fcc4eb2de656669d9ebdf37dee2ac3c51ee44b370cfb4d11bace4edef456eef1` | `list_browser_targets` | The site loads as a normal page target; the browser is Chrome/155 with protocol 1.3 (no UA override needed for REA to attach). |
| `ev_ec121d31b9539b88ada01c4aef5212d31c60d4c742859e12b0e080584f543f96` | `inspect_web_page` | `https://www.touchgal.ink/search` is a Next.js App Router document ("搜索 Galgame - TouchGal"); the passive capture is policy-filtered for DOM/script bodies but confirms target identity, origin and that the page is client-rendered (the result list is fetched after hydration). |

Notes on what REA could and could not establish here:

* `inspect_web_page` is passive — it never evaluates page JavaScript — so it
  cannot show the `POST /api/search` body. That body was obtained with a CDP
  `Network.*` capture (`research/`), which is why every payload in
  `reverse-engineering.md` is quoted from an actual request rather than from
  reading the source alone.
* `capture_browser_scenario` was attempted for the resource → `/redirect` →
  pan hop but timed out on the cross-origin navigation; the same hop was captured
  step by step instead (screenshot `3-redirect-interstitial.png` shows the
  interstitial with the encoded `url=` parameter).
* The headless browser is the only way to see the *rendered* result: the API
  answers JSON, but the counts and chips shown in `1-search-results.png`
  ("共 1 个结果", 8.5 stars, 1.9M views) confirm the JSON and the UI agree.
