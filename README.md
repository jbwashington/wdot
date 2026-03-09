# wdot

**W**eb **D**ynamic **O**utput **T**ool — a fast, stealthy headless browser built in Rust. Designed for AI agents that need to navigate dynamic web content with low token cost.

Renders JavaScript-heavy pages via headless Chrome, strips noise, and returns clean markdown. Auto-detects and solves captchas via [2Captcha](https://2captcha.com). Resists bot detection through comprehensive stealth evasions and TLS fingerprint randomization.

## Features

- **Dynamic content rendering** — Full Chromium engine via CDP, handles SPAs and JS-rendered pages
- **Token-efficient output** — Strips nav/header/footer/scripts, extracts `<main>`/`<article>`, converts to clean markdown
- **Stealth mode** — 14+ anti-detection evasions (navigator.webdriver, WebGL, plugins, user-agent, etc.)
- **TLS fingerprint evasion** — Cipher suite randomization via `PermuteTLSExtensions`, optional proxy for full JA3/JA4 spoofing
- **Auto captcha solving** — reCAPTCHA v2/v3, hCaptcha, Cloudflare Turnstile via 2Captcha API v2
- **Single binary** — ~14MB, no runtime dependencies beyond Chrome

## Quick start

```bash
# Build
cargo build --release

# Run (Chrome/Chromium must be installed)
./target/release/wdot

# Or install it
cargo install --path .

# Fetch a page
curl -X POST http://127.0.0.1:3100/fetch \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com"}'
```

## API

### `GET /health`

Returns `ok`.

### `POST /fetch`

Fetch a page and return clean markdown.

**Request body:**

```json
{
  "url": "https://example.com",
  "wait_for": ".content-loaded",
  "timeout_ms": 30000,
  "include_links": true
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | yes | URL to fetch |
| `wait_for` | string | no | CSS selector to wait for before extracting |
| `timeout_ms` | number | no | Timeout in ms (default: 30000) |
| `include_links` | bool | no | Include extracted links (default: false) |
| `max_tokens` | number | no | Cap output size (default: 50000). Truncates at paragraph boundaries |

**Response:**

```json
{
  "url": "https://example.com/",
  "title": "Example Domain",
  "markdown": "Example Domain\n==========\n\nThis domain is for use in...",
  "links": [
    {"text": "Learn more", "href": "https://iana.org/domains/example"}
  ],
  "token_estimate": 44
}
```

## Configuration

All configuration is via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `HOST` | `127.0.0.1` | Bind address |
| `PORT` | `3100` | Bind port |
| `HEADLESS` | `true` | Run Chrome headlessly |
| `STEALTH` | `true` | Enable stealth evasions |
| `CHROME_PATH` | (auto-detect) | Path to Chrome/Chromium binary |
| `TWOCAPTCHA_API_KEY` | | 2Captcha API key (enables auto captcha solving) |
| `PROXY_URL` | | Proxy for TLS fingerprint spoofing |

## Stealth evasions

When `STEALTH=true` (default), the following evasions are applied:

- `navigator.webdriver` returns `undefined`
- `window.chrome` runtime object present
- Realistic plugin array (Chrome PDF Plugin, etc.)
- WebGL vendor/renderer spoofed (Intel Iris)
- `navigator.languages`, `hardwareConcurrency`, `deviceMemory`, `platform` set to realistic values
- Permissions API patched
- `Function.prototype.toString` patched to hide overrides
- ChromeDriver `cdc_` variables removed
- `--disable-blink-features=AutomationControlled`
- TLS cipher suite randomization (`PermuteTLSExtensions`)
- Realistic user-agent string

## Captcha solving

With a `TWOCAPTCHA_API_KEY` set, wdot auto-detects and solves:

- **Cloudflare Turnstile** — explicit widgets, implicit scripts, and managed challenge pages
- **reCAPTCHA v2** — checkbox challenges
- **reCAPTCHA v3** — score-based challenges
- **hCaptcha** — interactive challenges

Uses the [2Captcha API v2](https://2captcha.com/api-docs) (`createTask`/`getTaskResult`).

## TLS fingerprint evasion

Chrome's TLS fingerprint (JA3/JA4) is well-known to bot detection services. wdot mitigates this in two layers:

1. **Built-in**: `PermuteTLSExtensions` flag randomizes cipher suite order in the TLS ClientHello
2. **Proxy mode**: Set `PROXY_URL` to route through a TLS-spoofing proxy like [curl-impersonate](https://github.com/lwthiker/curl-impersonate) for full JA3 mimicry

## Architecture

```
Agent (HTTP POST)
    |
    v
axum server (src/main.rs)
    |
    v
chromiumoxide (CDP) -----> stealth evasions injected per page
    |                       TLS fingerprint randomized
    v
HTML rendered with JS
    |
    v
extractor (src/extractor.rs)
    |--- strips <script>, <style>, <nav>, <footer>, etc.
    |--- prefers <main>/<article> content
    |--- converts to markdown via html2md
    v
clean markdown + links + token estimate
```

## License

MIT
