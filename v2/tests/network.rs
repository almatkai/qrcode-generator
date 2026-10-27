use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tezbar_extension_sdk::{BoxFuture, Extension, ExtensionPaths, TestContext};
use tezbar_qrcode_generator::{
    domain::Draft,
    shortener::{endpoints, shorten, Network, Transport},
    Adapter,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
async fn server(status: &str, body: Vec<u8>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let task = tokio::spawn(async move {
        let (mut c, _) = listener.accept().await.unwrap();
        let mut input = [0u8; 4096];
        let _ = c.read(&mut input).await;
        let _ = c.write_all(header.as_bytes()).await;
        let _ = c.write_all(&body).await;
    });
    (url, task)
}
#[tokio::test]
async fn bounded_real_http_client_local_only() {
    let network = Network::new().unwrap();
    for (status, body, ok) in [
        ("200 OK", b" https://short.test/ok \n".to_vec(), true),
        ("500 Error", b"https://short.test/fail".to_vec(), false),
        ("200 OK", b"error no".to_vec(), false),
        ("200 OK", b"mailto:no".to_vec(), false),
        ("200 OK", vec![b'a'; 4097], false),
        (
            "302 Found",
            b"https://short.test/no-redirect".to_vec(),
            false,
        ),
    ] {
        let (url, task) = server(status, body).await;
        assert_eq!(network.get(&url).await.is_ok(), ok);
        task.await.unwrap();
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), network.get(&url))
            .await
            .is_err()
    );
    task.abort();
    let _ = task.await;
    let targets = endpoints("https://example.test/?x=a b&中=1");
    assert!(targets[0].starts_with("https://is.gd/create.php?format=simple&url="));
    assert!(targets[1].starts_with("https://da.gd/shorten?url="));
    assert!(targets[2].starts_with("https://tinyurl.com/api-create.php?url="));
    assert!(targets[0].contains("%E4%B8%AD"));
}
struct Pending(AtomicUsize);
impl Transport for Pending {
    fn get<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<String, String>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::pending())
    }
}
#[tokio::test]
async fn dropped_generation_does_not_restart_or_reacquire() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let test = TestContext::new(
        ExtensionPaths {
            package_dir: root.clone(),
            data_dir: root.join("data"),
            temp_dir: root.join("temp"),
        },
        &[],
    );
    let network = Arc::new(Pending(AtomicUsize::new(0)));
    let a = Adapter::with_services(network.clone(), Some(root.join("Downloads")));
    let mut state = json!({});
    let mut payload = json!({"sessionId":"index","preferences":{}});
    a.handle(
        &test.context,
        "index",
        "extension:run-command",
        &payload,
        &mut state,
    )
    .await
    .unwrap();
    let mut fields = serde_json::to_value(Draft::new("#000000".into(), "png".into())).unwrap();
    fields["url"] = json!("https://example.test");
    fields["shorten"] = json!(true);
    fields["__tezbarDraftRevision"] = json!(1);
    payload["actionId"] = json!("generate-show");
    payload["formValues"] = fields;
    assert!(tokio::time::timeout(
        std::time::Duration::from_millis(20),
        a.handle(
            &test.context,
            "index",
            "extension:invoke-action",
            &payload,
            &mut state
        )
    )
    .await
    .is_err());
    assert_eq!(network.0.load(Ordering::SeqCst), 1);
    a.handle(
        &test.context,
        "index",
        "extension:refresh-session",
        &payload,
        &mut state,
    )
    .await
    .unwrap();
    let v = a
        .handle(
            &test.context,
            "index",
            "extension:invoke-action",
            &payload,
            &mut state,
        )
        .await
        .unwrap();
    assert!(v["message"].as_str().unwrap().contains("interrupted"));
    assert_eq!(network.0.load(Ordering::SeqCst), 1);
    assert!(test.calls().is_empty());
}
struct Failing(AtomicUsize);
impl Transport for Failing {
    fn get<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<String, String>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err("secret from remote must not appear".into()) })
    }
}
#[tokio::test]
async fn fallback_failure_is_safe() {
    let mock = Arc::new(Failing(AtomicUsize::new(0)));
    let network: Arc<dyn Transport> = mock.clone();
    let error = shorten(&network, "https://secret.test/password")
        .await
        .unwrap_err();
    assert_eq!(mock.0.load(Ordering::SeqCst), 3);
    assert!(!error.contains("secret"));
    assert!(!error.contains("password"));
}

struct FailThenPending(AtomicUsize);
impl Transport for FailThenPending {
    fn get<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<String, String>> {
        let n = self.0.fetch_add(1, Ordering::SeqCst);
        if n < 3 {
            Box::pin(async { Err("temporary outage".into()) })
        } else {
            Box::pin(std::future::pending())
        }
    }
}
#[tokio::test]
async fn cancelled_explicit_generation_retry_consumes_token_and_fences_queued_replay() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let test = TestContext::new(
        ExtensionPaths {
            package_dir: root.clone(),
            data_dir: root.join("data"),
            temp_dir: root.join("temp"),
        },
        &[],
    );
    let network = Arc::new(FailThenPending(AtomicUsize::new(0)));
    let a = Adapter::with_services(network.clone(), Some(root.join("Downloads")));
    let mut state = json!({});
    a.handle(
        &test.context,
        "index",
        "extension:run-command",
        &json!({"sessionId":"index"}),
        &mut state,
    )
    .await
    .unwrap();
    let mut fields = serde_json::to_value(Draft::new("#000000".into(), "png".into())).unwrap();
    fields["url"] = json!("https://example.test");
    fields["shorten"] = json!(true);
    fields["__tezbarDraftRevision"] = json!(1);
    let mut payload = json!({"sessionId":"index","actionId":"generate-show","formValues":fields});
    let failed = a
        .handle(
            &test.context,
            "index",
            "extension:invoke-action",
            &payload,
            &mut state,
        )
        .await
        .unwrap();
    let retry = failed["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"].as_str().unwrap().starts_with("retry-generation:"))
        .unwrap()["id"]
        .clone();
    payload["actionId"] = retry;
    let mut queued_state = json!({});
    let (cancelled, queued) = tokio::join!(
        tokio::time::timeout(
            std::time::Duration::from_millis(20),
            a.handle(
                &test.context,
                "index",
                "extension:invoke-action",
                &payload,
                &mut state
            )
        ),
        a.handle(
            &test.context,
            "index",
            "extension:invoke-action",
            &payload,
            &mut queued_state
        )
    );
    assert!(cancelled.is_err());
    assert!(queued.unwrap_err().contains("Expired generation retry"));
    assert_eq!(network.0.load(Ordering::SeqCst), 4);
    payload["actionId"] = json!("generate-show");
    a.handle(
        &test.context,
        "index",
        "extension:invoke-action",
        &payload,
        &mut state,
    )
    .await
    .unwrap();
    let v = a
        .handle(
            &test.context,
            "index",
            "extension:refresh-session",
            &payload,
            &mut state,
        )
        .await
        .unwrap();
    assert!(v["message"].as_str().unwrap().contains("interrupted"));
    assert!(!v["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["id"].as_str().unwrap().starts_with("retry-generation:")));
    assert_eq!(network.0.load(Ordering::SeqCst), 4);
    assert!(test.calls().is_empty());
}
