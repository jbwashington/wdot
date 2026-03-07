use chromiumoxide::page::Page;

/// Apply all stealth evasions to a page before navigation completes.
/// These counter the most common headless detection techniques used by
/// bot-protection services (Cloudflare, DataDome, PerimeterX, etc).
pub async fn apply(page: &Page) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Run all evasion scripts. Order matters — some override things others set.
    // We use evaluate_on_new_document where possible via a single injection.
    page.evaluate(STEALTH_SCRIPT).await?;
    Ok(())
}

const STEALTH_SCRIPT: &str = r#"
(() => {
    // 1. navigator.webdriver — the #1 headless detection signal
    Object.defineProperty(navigator, 'webdriver', {
        get: () => undefined,
    });

    // 2. Chrome runtime — headless Chrome lacks window.chrome
    if (!window.chrome) {
        window.chrome = {
            runtime: {},
            loadTimes: function() {},
            csi: function() {},
            app: { isInstalled: false, InstallState: { DISABLED: "disabled", INSTALLED: "installed", NOT_INSTALLED: "not_installed" }, RunningState: { CANNOT_RUN: "cannot_run", READY_TO_RUN: "ready_to_run", RUNNING: "running" } },
        };
    }

    // 3. Permissions API — headless returns "prompt" for notifications instead of "denied"
    const originalQuery = window.navigator.permissions.query;
    window.navigator.permissions.query = (parameters) => (
        parameters.name === 'notifications'
            ? Promise.resolve({ state: Notification.permission })
            : originalQuery(parameters)
    );

    // 4. Plugin array — headless Chrome has 0 plugins
    Object.defineProperty(navigator, 'plugins', {
        get: () => {
            const plugins = [
                { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
                { name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai', description: '' },
                { name: 'Native Client', filename: 'internal-nacl-plugin', description: '' },
            ];
            plugins.refresh = () => {};
            return plugins;
        },
    });

    // 5. Languages — headless often has empty languages array
    Object.defineProperty(navigator, 'languages', {
        get: () => ['en-US', 'en'],
    });

    // 6. WebGL vendor/renderer — headless exposes "Google SwiftShader"
    const getParameter = WebGLRenderingContext.prototype.getParameter;
    WebGLRenderingContext.prototype.getParameter = function(parameter) {
        if (parameter === 37445) return 'Intel Inc.';          // UNMASKED_VENDOR_WEBGL
        if (parameter === 37446) return 'Intel Iris OpenGL Engine'; // UNMASKED_RENDERER_WEBGL
        return getParameter.call(this, parameter);
    };
    const getParameter2 = WebGL2RenderingContext.prototype.getParameter;
    WebGL2RenderingContext.prototype.getParameter = function(parameter) {
        if (parameter === 37445) return 'Intel Inc.';
        if (parameter === 37446) return 'Intel Iris OpenGL Engine';
        return getParameter2.call(this, parameter);
    };

    // 7. Hairline feature — headless doesn't support CSS hairlines
    if (!('ontouchstart' in window)) {
        // Only patch on non-touch devices (desktop headless)
        const div = document.createElement('div');
        div.style.border = '.5px solid transparent';
        document.body?.appendChild(div);
        if (div.offsetHeight === 1) {
            // Already supports hairlines
        }
        div.remove?.();
    }

    // 8. Connection rtt — headless returns 0
    if (navigator.connection) {
        Object.defineProperty(navigator.connection, 'rtt', { get: () => 50 });
    }

    // 9. Hardware concurrency — headless sometimes returns weird values
    Object.defineProperty(navigator, 'hardwareConcurrency', {
        get: () => 8,
    });

    // 10. Device memory
    Object.defineProperty(navigator, 'deviceMemory', {
        get: () => 8,
    });

    // 11. Platform consistency
    Object.defineProperty(navigator, 'platform', {
        get: () => 'MacIntel',
    });

    // 12. iframe contentWindow — headless leaks on cross-origin frames
    try {
        const origContentWindow = Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype, 'contentWindow');
        if (origContentWindow) {
            Object.defineProperty(HTMLIFrameElement.prototype, 'contentWindow', {
                get: function() {
                    const w = origContentWindow.get.call(this);
                    if (!w) return w;
                    // Patch the iframe's navigator.webdriver too
                    try { Object.defineProperty(w.navigator, 'webdriver', { get: () => undefined }); } catch(e) {}
                    return w;
                }
            });
        }
    } catch(e) {}

    // 13. Prevent toString detection — sites check if functions were overridden
    const nativeToString = Function.prototype.toString;
    const overrides = new Map();

    function patchToString(obj, prop, fakeStr) {
        const orig = Object.getOwnPropertyDescriptor(obj, prop);
        if (orig && orig.get) {
            overrides.set(orig.get, fakeStr || `function get ${prop}() { [native code] }`);
        }
    }

    Function.prototype.toString = function() {
        if (overrides.has(this)) return overrides.get(this);
        return nativeToString.call(this);
    };
    // Hide our own toString override
    overrides.set(Function.prototype.toString, 'function toString() { [native code] }');

    patchToString(navigator, 'webdriver');
    patchToString(navigator, 'plugins');
    patchToString(navigator, 'languages');
    patchToString(navigator, 'hardwareConcurrency');
    patchToString(navigator, 'deviceMemory');
    patchToString(navigator, 'platform');

    // 14. Mask automation-controlled window properties
    delete window.cdc_adoQpoasnfa76pfcZLmcfl_Array;
    delete window.cdc_adoQpoasnfa76pfcZLmcfl_Promise;
    delete window.cdc_adoQpoasnfa76pfcZLmcfl_Symbol;
})();
"#;

/// Chrome launch args that reduce the headless fingerprint.
pub fn stealth_args() -> Vec<&'static str> {
    vec![
        "--disable-blink-features=AutomationControlled",
        "--disable-features=IsolateOrigins,site-per-process",
        "--disable-infobars",
        "--window-size=1920,1080",
        "--start-maximized",
        "--disable-backgrounding-occluded-windows",
        "--disable-renderer-backgrounding",
    ]
}

/// A realistic user-agent string.
pub fn user_agent() -> &'static str {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
}
