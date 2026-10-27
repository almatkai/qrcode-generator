use crate::domain::{http, js_trim};
use reqwest::{redirect::Policy, Client};
use std::{sync::Arc, time::Duration};
use tezbar_extension_sdk::BoxFuture;

pub trait Transport: Send + Sync {
    fn get<'a>(&'a self, endpoint: &'a str) -> BoxFuture<'a, Result<String, String>>;
}
pub struct Network(Client);
impl Network {
    pub fn new() -> Result<Self, String> {
        Client::builder()
            .timeout(Duration::from_secs(8))
            .connect_timeout(Duration::from_secs(4))
            .redirect(Policy::none())
            .no_proxy()
            .build()
            .map(Self)
            .map_err(|_| "HTTP client initialization failed".into())
    }
}
impl Transport for Network {
    fn get<'a>(&'a self, endpoint: &'a str) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let result = tokio::time::timeout(Duration::from_secs(8), async {
                let mut response = self
                    .0
                    .get(endpoint)
                    .send()
                    .await
                    .map_err(|_| "Request failed")?;
                if !response.status().is_success()
                    || response.content_length().is_some_and(|n| n > 4096)
                {
                    return Err("Shortener rejected request".into());
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(|_| "Response failed")? {
                    if bytes.len() + chunk.len() > 4096 {
                        return Err("Shortener response too large".into());
                    }
                    bytes.extend_from_slice(&chunk);
                }
                let body =
                    String::from_utf8(bytes).map_err(|_| "Shortener response is not UTF-8")?;
                let text = js_trim(&body);
                if text.to_ascii_lowercase().starts_with("error") || http(text).is_none() {
                    return Err("Invalid shortener response".into());
                }
                Ok(text.to_owned())
            })
            .await;
            result.unwrap_or_else(|_| Err("Shortener timed out".into()))
        })
    }
}
pub fn endpoints(value: &str) -> [String; 3] {
    let escaped = tezbar_extension_sdk::encode_component(value);
    [
        format!("https://is.gd/create.php?format=simple&url={escaped}"),
        format!("https://da.gd/shorten?url={escaped}"),
        format!("https://tinyurl.com/api-create.php?url={escaped}"),
    ]
}
pub async fn shorten(network: &Arc<dyn Transport>, value: &str) -> Result<String, String> {
    for endpoint in endpoints(value) {
        // One bounded requested operation, V1 fallback order. Errors never echo endpoints/content.
        if let Ok(body) = network.get(&endpoint).await {
            let text = js_trim(&body);
            if text.len() <= 4096
                && !text.to_ascii_lowercase().starts_with("error")
                && http(text).is_some()
            {
                return Ok(text.into());
            }
        }
    }
    Err("Could not shorten link (is.gd, da.gd, TinyURL failed); generation stopped".into())
}
