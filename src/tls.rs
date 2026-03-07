/// TLS fingerprint evasion via Chrome launch flags.
///
/// Chrome's default TLS fingerprint (JA3/JA4) is well-known to bot detection
/// services. There are several mitigation strategies:
///
/// 1. **Cipher suite shuffling** — Chrome 110+ supports this natively via flags
/// 2. **Proxy through curl-impersonate** — external proxy that mimics real browser TLS
/// 3. **Custom proxy** — route through a proxy that normalizes TLS fingerprints
///
/// This module provides Chrome flags for option 1, and proxy config for options 2/3.

/// Chrome args that modify the TLS fingerprint to be harder to detect.
pub fn tls_evasion_args() -> Vec<&'static str> {
    vec![
        // Enable TLS ClientHello permutation (cipher suite randomization)
        // Available in Chrome 110+. Shuffles the order of cipher suites
        // in the ClientHello, breaking static JA3 fingerprinting.
        "--enable-features=PermuteTLSExtensions",
        // Disable TLS 1.0/1.1 to avoid unusual fingerprint from legacy support
        "--ssl-version-min=tls1.2",
        // Reduce GREASE usage patterns that identify headless Chrome
        "--disable-features=TLSExtensionGREASE",
    ]
}

/// Generate Chrome flags to route traffic through a proxy.
/// Use this with curl-impersonate-chrome or similar TLS-spoofing proxies.
///
/// Example: run curl-impersonate as a local MITM proxy, then pass
/// `--proxy-server=http://127.0.0.1:8080` to Chrome.
pub fn proxy_arg(proxy_url: &str) -> String {
    format!("--proxy-server={}", proxy_url)
}

/// Flags to ignore cert errors when using a MITM proxy for TLS spoofing.
/// Only use this with a local trusted proxy (like curl-impersonate).
pub fn proxy_cert_args() -> Vec<&'static str> {
    vec![
        "--ignore-certificate-errors",
        "--allow-insecure-localhost",
    ]
}
