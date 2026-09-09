use serde_json::{json, Value};

pub async fn handle(method: &str, _params: &Value) -> Result<Value, String> {
    match method {
        "getVersion" => Ok(json!({
            "protocolVersion": "1.3",
            "product": "Chrome/149.0.0.0",
            "revision": "@0000000000000000000000000000000000000000",
            "userAgent": obscura_net::BROWSER_USER_AGENT,
            "jsVersion": "14.5.0.0",
        })),
        "close" => {
            Ok(json!({}))
        }
        "getWindowForTarget" => Ok(json!({
            "windowId": 1,
            "bounds": {
                "left": 0,
                "top": 0,
                "width": 1280,
                "height": 720,
                "windowState": "normal",
            }
        })),
        "setDownloadBehavior" => Ok(json!({})),
        "getWindowBounds" => Ok(json!({
            "bounds": { "left": 0, "top": 0, "width": 1280, "height": 720, "windowState": "normal" }
        })),
        // No-op acks for window-management methods Playwright sends during
        // page setup. We don't model real OS windows, but answering with {}
        // lets the client's setup sequence complete instead of tearing down
        // the page on an unknown-method error.
        "setWindowBounds" => Ok(json!({})),
        // Playwright grants permissions (geolocation, notifications, ...) per
        // browser context during setup. obscura does not gate any API on a
        // permission grant today, so the honest answer is to accept and
        // remember nothing; an unknown-method error would abort the client's
        // whole context initialization.
        "grantPermissions" | "resetPermissions" => Ok(json!({})),
        _ => Err(format!("Unknown Browser method: {}", method)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_version_uses_supported_windows_identity() {
        let version = handle("getVersion", &Value::Null).await.unwrap();
        assert_eq!(version["product"], "Chrome/149.0.0.0");
        assert_eq!(version["userAgent"], obscura_net::BROWSER_USER_AGENT);
    }
}
