# wdot

**W**eb **D**ynamic **O**utput **T**ool — a fast, stealthy headless browser built in Rust for AI agents, OSINT, and web automation.

Renders JavaScript-heavy pages via headless Chrome, strips noise, and returns clean markdown with minimal token cost. Learns from human browsing patterns to evade bot detection. Monitors its own reputation and adapts automatically. Auto-detects and solves captchas via [2Captcha](https://2captcha.com).

## Features

- **Dynamic content rendering** — Full Chromium engine via CDP, handles SPAs and JS-rendered pages
- **Token-efficient output** — Strips nav/header/footer/scripts, extracts `<main>`/`<article>`, converts to clean markdown (99% noise reduction on complex pages)
- **Stealth mode** — 14+ anti-detection evasions (navigator.webdriver, WebGL, plugins, user-agent, etc.)
- **TLS fingerprint evasion** — Cipher suite randomization via `PermuteTLSExtensions`, optional proxy for full JA3/JA4 spoofing
- **Auto captcha solving** — reCAPTCHA v2/v3, hCaptcha, Cloudflare Turnstile via 2Captcha API v2
- **OSINT engine** — Email harvesting, social profile detection, DNS enumeration, technology fingerprinting, metadata extraction, document discovery
- **Human behavior mimicry** — Records and replays browsing patterns (mouse movements, scrolling, typing cadence) with statistical noise to appear human
- **Reputation monitoring** — Tracks captcha/block/challenge rates, computes reputation score with trend analysis, auto-adjusts delays and rotates fingerprints
- **Headful mode** — Set `HEADLESS=false` for form fills, social media posting, and browser automation with visible Chrome
- **Single binary** — No runtime dependencies beyond Chrome

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

### Core

#### `GET /health`

Returns `ok`.

#### `POST /fetch`

Fetch a page and return clean markdown.

```json
{
  "url": "https://example.com",
  "wait_for": ".content-loaded",
  "timeout_ms": 30000,
  "include_links": true,
  "max_tokens": 50000
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | yes | URL to fetch |
| `wait_for` | string | no | CSS selector to wait for before extracting |
| `timeout_ms` | number | no | Timeout in ms (default: 30000) |
| `include_links` | bool | no | Include extracted links (default: false) |
| `max_tokens` | number | no | Cap output size. Truncates at paragraph boundaries |

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

### OSINT

#### `POST /osint/scan`

Run a full OSINT scan on a target URL. Returns emails, social profiles, DNS info, metadata, documents, and technologies.

```json
{"target": "https://example.com"}
```

#### `POST /osint/emails`

Extract email addresses from a target URL.

```json
{"target": "https://example.com"}
```

#### `POST /osint/tech`

Detect technologies used by a website (frameworks, CMS, analytics, CDNs).

```json
{"target": "https://example.com"}
```

#### `POST /osint/dns`

Perform DNS enumeration for a domain — A, AAAA, MX, TXT, NS, CNAME records.

```json
{"domain": "example.com"}
```

### Behavior engine

Record human browsing patterns and replay them to appear indistinguishable from a real user.

#### `POST /behavior/record/start`

Start recording a new browsing profile.

```json
{"name": "my-profile"}
```

#### `POST /behavior/record/stop`

Stop recording and save the profile.

#### `GET /behavior/profiles`

List all saved behavior profiles.

#### `POST /behavior/profiles/:name/activate`

Activate a behavior profile. All subsequent requests will mimic that profile's mouse movements, scroll patterns, and timing.

#### `DELETE /behavior/profiles/:name`

Delete a behavior profile.

### Reputation monitoring

Monitor bot-detection reputation in real time. The system tracks captcha encounters, blocks, and challenge redirects across a sliding window and adapts behavior automatically.

#### `GET /reputation`

Get current reputation score and adaptive state.

**Response:**

```json
{
  "score": {
    "overall": 0.95,
    "captcha_rate": 2.0,
    "block_rate": 0.0,
    "challenge_rate": 1.0,
    "trend": "Stable",
    "window_size": 50,
    "total_requests": 50
  },
  "adaptive": {
    "current_delay_ms": 100,
    "requests_since_rotation": 12,
    "should_rotate_fingerprint": false,
    "paused": false,
    "alert_level": "Green"
  }
}
```

**Alert levels:**

| Level | Score range | Action |
|-------|------------|--------|
| Green | > 0.8 | Normal operation |
| Yellow | 0.6 - 0.8 | Increased delays (1.5x) |
| Orange | 0.4 - 0.6 | Fingerprint rotation, 2x delays |
| Red | 0.2 - 0.4 | Aggressive adaptation, 3x delays |
| Critical | < 0.2 | Paused, 60s cooldown |

#### `GET /reputation/history`

Get the last 50 session signals for analysis.

#### `POST /reputation/reset`

Reset all reputation tracking data.

#### `GET /reputation/config`

Get the current adaptive configuration.

#### `PUT /reputation/config`

Update adaptive thresholds.

```json
{
  "min_delay_ms": 200,
  "max_delay_ms": 15000,
  "fingerprint_rotation_interval": 30,
  "cooldown_duration_secs": 120
}
```

## Configuration

All configuration is via environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `HOST` | `127.0.0.1` | Bind address |
| `PORT` | `3100` | Bind port |
| `HEADLESS` | `true` | Run Chrome headlessly (`false` for visible browser) |
| `STEALTH` | `true` | Enable stealth evasions |
| `CHROME_PATH` | (auto-detect) | Path to Chrome/Chromium binary |
| `TWOCAPTCHA_API_KEY` | | 2Captcha API key (enables auto captcha solving) |
| `PROXY_URL` | | Proxy for TLS fingerprint spoofing |
| `WDOT_DATA_DIR` | `~/.wdot` | Data directory for behavior profiles |
| `BEHAVIOR_PROFILE` | | Auto-activate a behavior profile on startup |
| `REPUTATION` | `true` | Enable reputation monitoring |
| `REPUTATION_WINDOW` | `200` | Number of signals in the scoring window (~25KB memory) |

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

## OSINT capabilities

The `/osint/scan` endpoint runs all extractors in parallel and returns a comprehensive report:

- **Emails** — Regex pattern matching + `mailto:` link harvesting with false positive filtering
- **Social profiles** — URL pattern matching against 16 platforms (Twitter, LinkedIn, GitHub, Facebook, Instagram, YouTube, etc.) + `<link rel="me">` IndieWeb standard
- **DNS** — Full enumeration via hickory-resolver (A, AAAA, MX, TXT, NS, CNAME)
- **Technologies** — HTML meta tags, script/CSS patterns, and framework signatures (React, Next.js, Vue, Angular, WordPress, Shopify, etc.)
- **Metadata** — Title, description, canonical URL, OpenGraph, Twitter Cards, all meta tags
- **Documents** — Discovery of linked files by extension (.pdf, .doc, .xls, .csv, .zip, etc.)

## Human behavior mimicry

The behavior engine learns from real browsing sessions and replays them statistically:

- **Mouse movements** — Bezier curve paths with curvature variation, jitter, and overshoot simulation
- **Scrolling** — Burst patterns with natural pauses, variable speed
- **Typing** — Per-character delay distributions with occasional typos and corrections
- **Timing** — Navigation delays, dwell time, first-action delays sampled from statistical distributions
- **Profiles** — Saved as compact bincode files (~1KB each) in `~/.wdot/profiles/`

## Architecture

```
Agent (HTTP POST)
    |
    v
axum server -----> reputation monitor (ring buffer, ~25KB)
    |                  |-> adaptive delays
    |                  |-> fingerprint rotation signals
    |                  |-> auto-pause on critical score
    v
behavior engine -----> pre-navigation delay
    |                   |-> reading simulation (scroll + mouse)
    |                   |-> typing with human cadence
    v
chromiumoxide (CDP) -> stealth evasions (14+ techniques)
    |                   |-> TLS fingerprint randomization
    |                   |-> captcha auto-solve (2Captcha API v2)
    v
HTML rendered with JS
    |
    v
extractor -----------> noise removal (60+ selectors)
    |                   |-> semantic content extraction
    |                   |-> table layout fallback
    |                   |-> CSS artifact stripping
    v
clean markdown + links + token estimate
    |
    v
OSINT engine --------> emails, social, DNS, tech, metadata, docs
```

## License

MIT
