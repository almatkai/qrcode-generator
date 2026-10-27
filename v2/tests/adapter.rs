use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tezbar_extension_sdk::{BoxFuture, Extension, ExtensionPaths, TestContext};
use tezbar_qrcode_generator::{acknowledgement, domain::Draft, shortener::Transport, Adapter};
#[derive(Default)]
struct Mock {
    calls: Mutex<Vec<String>>,
    replies: Mutex<Vec<Result<String, String>>>,
}
impl Transport for Mock {
    fn get<'a>(&'a self, s: &'a str) -> BoxFuture<'a, Result<String, String>> {
        self.calls.lock().unwrap().push(s.into());
        Box::pin(async move { self.replies.lock().unwrap().remove(0) })
    }
}
fn fields(text: &str) -> Value {
    let mut v = serde_json::to_value(Draft::new("#000000".into(), "png".into())).unwrap();
    v["url"] = json!(text);
    v["__tezbarDraftRevision"] = json!(1);
    v
}
async fn call(a: &Adapter, c: &TestContext, command: &str, op: &str, extra: Value) -> Value {
    let mut p = json!({"sessionId":command,"preferences":{}});
    for (k, v) in extra.as_object().unwrap() {
        p[k] = v.clone();
    }
    let mut state = json!({});
    c.run(a.handle(&c.context, command, op, &p, &mut state))
        .await
        .unwrap()
}
fn context(dir: &std::path::Path) -> TestContext {
    TestContext::new(
        ExtensionPaths {
            package_dir: dir.join("package"),
            data_dir: dir.join("data"),
            temp_dir: dir.join("temp"),
        },
        &[
            "clipboard.read",
            "selection.read",
            "clipboard.writeImage",
            "shell.open",
        ],
    )
}
#[tokio::test]
async fn index_order_forms_warnings_actions_and_draft() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let c = context(&root);
    let network = Arc::new(Mock::default());
    let a = Adapter::with_services(network.clone(), Some(root.join("Downloads")));
    let first = call(
        &a,
        &c,
        "index",
        "extension:run-command",
        json!({"preferences":{"defaultFormat":"svg","defaultColor":"AbC","primaryAction":"copy"}}),
    )
    .await;
    assert_eq!(first["actions"][0]["id"], "generate-copy");
    assert_eq!(first["root"]["children"][1]["props"]["value"], "");
    assert_eq!(first["root"]["children"][3]["props"]["value"], "#AAbbCC");
    assert!(c.calls().is_empty());
    let mut f = fields("hello");
    f["utmEnabled"] = json!(true);
    f["customColor"] = json!("#fff");
    let v = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"field:utmEnabled","formValues":f}),
    )
    .await;
    assert_eq!(v["root"]["children"].as_array().unwrap().len(), 13);
    assert!(v["root"]["children"][2]["props"]["error"]
        .as_str()
        .unwrap()
        .contains("Low contrast"));
    // Older complete map cannot overwrite current draft.
    let mut old = fields("stale");
    old["__tezbarDraftRevision"] = json!(0);
    let v = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"field:url","formValues":old}),
    )
    .await;
    assert_eq!(v["root"]["children"][0]["props"]["value"], "hello");
    let mut f = fields("hello");
    f["__tezbarDraftRevision"] = json!(2);
    let v = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-show","formValues":f}),
    )
    .await;
    assert_eq!(v["root"]["type"], "Detail");
    assert!(v["root"]["props"]["markdown"]
        .as_str()
        .unwrap()
        .starts_with("![qrcode](data:image/png;base64,"));
    assert!(!v["root"]["props"]["markdown"]
        .as_str()
        .unwrap()
        .contains("raycast-height"));
    assert!(c.calls().is_empty());
    let v = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"edit"}),
    )
    .await;
    assert_eq!(v["root"]["children"][0]["props"]["value"], "hello");
    assert!(network.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn acquisition_once_raw_white_no_automatic_effects() {
    for command in ["clipboard", "selection"] {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().canonicalize().unwrap();
        let c = context(&root);
        c.reply(Ok(json!("  raw\n🙂  ")));
        let a = Adapter::with_services(Arc::new(Mock::default()), Some(root.join("Downloads")));
        let v = call(&a, &c, command, "extension:run-command", json!({})).await;
        assert_eq!(c.calls().len(), 1);
        assert_eq!(v["actions"][0]["title"], "Save to Downloads");
        assert_eq!(v["actions"][1]["title"], "Copy to Clipboard");
        let again = call(&a, &c, command, "extension:refresh-session", json!({})).await;
        assert_eq!(v, again);
        assert_eq!(c.calls().len(), 1);
        let save = v["actions"][0]["id"].as_str().unwrap();
        call(
            &a,
            &c,
            command,
            "extension:invoke-action",
            json!({"actionId":save}),
        )
        .await;
        assert_eq!(c.calls().len(), 1);
        c.reply(Ok(json!({"ok":true})));
        let copy = v["actions"][1]["id"].as_str().unwrap();
        call(
            &a,
            &c,
            command,
            "extension:invoke-action",
            json!({"actionId":copy}),
        )
        .await;
        assert_eq!(c.calls().len(), 2);
        assert_eq!(c.calls()[1]["method"], "host.clipboard.writeImage");
        assert!(c.calls()[1]["params"]["file"]
            .as_str()
            .unwrap()
            .ends_with(".png"));
        call(
            &a,
            &c,
            command,
            "extension:invoke-action",
            json!({"actionId":copy}),
        )
        .await;
        assert_eq!(c.calls().len(), 2);
        assert_eq!(
            std::fs::read_dir(root.join("data/clipboard-staging"))
                .unwrap()
                .count(),
            0
        );
        call(&a, &c, command, "extension:dispose-session", json!({})).await;
    }
}
#[tokio::test]
async fn shortening_fallback_explicit_only_and_no_replay_after_open_failure() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let c = context(&root);
    let mock = Arc::new(Mock::default());
    mock.replies.lock().unwrap().extend([
        Err("outage".into()),
        Ok("error x".into()),
        Ok("https://short.test/x".into()),
    ]);
    let a = Adapter::with_services(mock.clone(), Some(root.join("Downloads")));
    call(&a, &c, "index", "extension:run-command", json!({})).await;
    let mut f = fields("https://example.com");
    f["shorten"] = json!(true);
    f["utmEnabled"] = json!(true);
    f["utmSource"] = json!("news letter");
    call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"field:shorten","formValues":f}),
    )
    .await;
    assert!(mock.calls.lock().unwrap().is_empty());
    c.reply(Ok(Value::Null));
    let v = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-save","formValues":f}),
    )
    .await;
    assert_eq!(mock.calls.lock().unwrap().len(), 3);
    assert!(mock.calls.lock().unwrap()[0].contains("utm_source%3Dnews%2Bletter"));
    let files: Vec<_> = std::fs::read_dir(root.join("Downloads"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    let bytes = std::fs::read(&files[0]).unwrap();
    let retry = v["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"].as_str().unwrap().starts_with("retry-open:"))
        .unwrap()["id"]
        .clone();
    c.reply(Ok(json!({"ok":true})));
    call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":retry}),
    )
    .await;
    assert_eq!(std::fs::read(&files[0]).unwrap(), bytes);
    assert_eq!(mock.calls.lock().unwrap().len(), 3);
    call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-save","formValues":f}),
    )
    .await;
    assert_eq!(c.calls().len(), 2);
    assert_eq!(mock.calls.lock().unwrap().len(), 3);
}
#[test]
fn acks_strict() {
    for v in [
        Value::Null,
        json!(true),
        json!(false),
        json!({"ok":false}),
        json!({"ok":true,"error":"failure"}),
        json!({"ok":true,"success":false}),
        json!({"ok":true,"cancelled":true}),
    ] {
        assert!(!acknowledgement(&v));
    }
    assert!(acknowledgement(&json!({"ok":true})));
}
#[tokio::test]
async fn svg_save_is_real_copy_limitation_and_missing_maps_fail_closed() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let c = context(&root);
    let mock = Arc::new(Mock::default());
    let a = Adapter::with_services(mock.clone(), Some(root.join("Downloads")));
    call(&a, &c, "index", "extension:run-command", json!({})).await;
    let mut f = fields("SVG input 🙂");
    f["format"] = json!("svg");
    let v = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-copy","formValues":f}),
    )
    .await;
    assert!(v["message"]
        .as_str()
        .unwrap()
        .starts_with("Failed to copy SVG"));
    assert!(c.calls().is_empty());
    c.reply(Ok(json!({"ok":true,"message":"Opened"})));
    call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-save","formValues":f}),
    )
    .await;
    let files: Vec<_> = std::fs::read_dir(root.join("Downloads"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].extension().unwrap(), "svg");
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert!(text.starts_with("<svg"));
    assert!(!text.contains("SVG input"));
    assert_eq!(c.calls().len(), 1);
    assert_eq!(c.calls()[0]["method"], "host.shell.open");
    assert!(mock.calls.lock().unwrap().is_empty());
    let mut state = json!({});
    let payload =
        json!({"sessionId":"index","actionId":"generate-save","formValues":{"url":"stale"}});
    assert!(a
        .handle(
            &c.context,
            "index",
            "extension:invoke-action",
            &payload,
            &mut state
        )
        .await
        .is_err());
}
#[tokio::test]
async fn png_capability_denial_does_not_stage_a_file() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let c = TestContext::new(
        ExtensionPaths {
            package_dir: root.clone(),
            data_dir: root.join("data"),
            temp_dir: root.join("temp"),
        },
        &[],
    );
    let a = Adapter::with_services(Arc::new(Mock::default()), Some(root.join("Downloads")));
    call(&a, &c, "index", "extension:run-command", json!({})).await;
    let f = fields("text");
    let v = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-copy","formValues":f}),
    )
    .await;
    assert!(v["message"].as_str().unwrap().contains("not granted"));
    assert!(!root.join("data/clipboard-staging").exists());
    assert!(c.calls().is_empty());
}
#[tokio::test]
async fn newer_queued_draft_fences_preview_and_old_prepared_actions() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let c = context(&root);
    let a = Adapter::with_services(Arc::new(Mock::default()), Some(root.join("Downloads")));
    call(&a, &c, "index", "extension:run-command", json!({})).await;
    let v = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-show","formValues":fields("old preview")}),
    )
    .await;
    assert_eq!(v["root"]["props"]["__tezbarDraftRevision"], 1);
    let mut f = fields("newer local draft");
    f["__tezbarDraftRevision"] = json!(2);
    let latest = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"field:url","formValues":f}),
    )
    .await;
    assert_eq!(latest["root"]["type"], "Form");
    assert_eq!(
        latest["root"]["children"][0]["props"]["value"],
        "newer local draft"
    );
    let mut state = json!({});
    assert!(a
        .handle(
            &c.context,
            "index",
            "extension:invoke-action",
            &json!({"sessionId":"index","actionId":"copy:discarded-id"}),
            &mut state
        )
        .await
        .is_err());
    assert!(c.calls().is_empty());
}

#[tokio::test]
async fn edited_a_b_a_regenerates_without_field_or_refresh_effects() {
    for intermediate in ["B", ""] {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().canonicalize().unwrap();
        let c = context(&root);
        let mock = Arc::new(Mock::default());
        mock.replies.lock().unwrap().extend([
            Ok("https://short.test/a".into()),
            Ok("https://short.test/a".into()),
        ]);
        let a = Adapter::with_services(mock.clone(), Some(root.join("Downloads")));
        call(&a, &c, "index", "extension:run-command", json!({})).await;
        let mut f = fields("https://example.test/A");
        f["shorten"] = json!(true);
        let first = call(
            &a,
            &c,
            "index",
            "extension:invoke-action",
            json!({"actionId":"generate-show","formValues":f}),
        )
        .await;
        assert_eq!(mock.calls.lock().unwrap().len(), 1);
        call(
            &a,
            &c,
            "index",
            "extension:invoke-action",
            json!({"actionId":"edit"}),
        )
        .await;
        // A real token from the discarded prepared output, not a fabricated UUID.
        c.reply(Ok(Value::Null));
        let failed_copy = call(
            &a,
            &c,
            "index",
            "extension:invoke-action",
            json!({"actionId":"generate-copy","formValues":f}),
        )
        .await;
        let stale = failed_copy["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"].as_str().unwrap().starts_with("retry-copy:"))
            .unwrap()["id"]
            .clone();
        let before = c.calls().len();
        let mut edit = f.clone();
        edit["url"] = json!(intermediate);
        edit["__tezbarDraftRevision"] = json!(2);
        call(
            &a,
            &c,
            "index",
            "extension:invoke-action",
            json!({"actionId":"field:url","formValues":edit}),
        )
        .await;
        f["__tezbarDraftRevision"] = json!(3);
        call(
            &a,
            &c,
            "index",
            "extension:invoke-action",
            json!({"actionId":"field:url","formValues":f}),
        )
        .await;
        call(&a, &c, "index", "extension:refresh-session", json!({})).await;
        assert_eq!(mock.calls.lock().unwrap().len(), 1);
        let mut state = json!({});
        assert!(a
            .handle(
                &c.context,
                "index",
                "extension:invoke-action",
                &json!({"sessionId":"index","actionId":stale}),
                &mut state
            )
            .await
            .is_err());
        let again = call(
            &a,
            &c,
            "index",
            "extension:invoke-action",
            json!({"actionId":"generate-show","formValues":f}),
        )
        .await;
        assert_eq!(again["root"]["type"], "Detail");
        assert_eq!(
            again["root"]["props"]["markdown"],
            first["root"]["props"]["markdown"]
        );
        assert_eq!(mock.calls.lock().unwrap().len(), 2);
        assert_eq!(c.calls().len(), before);
    }
}

#[tokio::test]
async fn completed_shortener_failure_issues_single_use_explicit_retry() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().canonicalize().unwrap();
    let c = context(&root);
    let mock = Arc::new(Mock::default());
    mock.replies.lock().unwrap().extend([
        Err("outage".into()),
        Err("outage".into()),
        Err("outage".into()),
        Ok("https://short.test/recovered".into()),
    ]);
    let a = Adapter::with_services(mock.clone(), Some(root.join("Downloads")));
    call(&a, &c, "index", "extension:run-command", json!({})).await;
    let mut f = fields("https://example.test");
    f["shorten"] = json!(true);
    let failed = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-show","formValues":f}),
    )
    .await;
    assert_eq!(mock.calls.lock().unwrap().len(), 3);
    let retry = failed["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"].as_str().unwrap().starts_with("retry-generation:"))
        .expect("completed failure must offer explicit recovery")["id"]
        .clone();
    // Queued ordinary submission and refresh are not evidence of retry intent.
    call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-show","formValues":f}),
    )
    .await;
    call(&a, &c, "index", "extension:refresh-session", json!({})).await;
    assert_eq!(mock.calls.lock().unwrap().len(), 3);
    let recovered = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":retry,"formValues":f}),
    )
    .await;
    assert_eq!(recovered["root"]["type"], "Detail");
    assert_eq!(mock.calls.lock().unwrap().len(), 4);
    let mut state = json!({});
    assert!(a
        .handle(
            &c.context,
            "index",
            "extension:invoke-action",
            &json!({"sessionId":"index","actionId":retry,"formValues":f}),
            &mut state
        )
        .await
        .is_err());
    call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"edit"}),
    )
    .await;
    c.reply(Ok(Value::Null));
    let copy = call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-copy","formValues":f}),
    )
    .await;
    let path = c.calls()[0]["params"]["file"].as_str().unwrap().to_owned();
    let bytes = std::fs::read(&path).unwrap();
    let retry_copy = copy["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"].as_str().unwrap().starts_with("retry-copy:"))
        .unwrap()["id"]
        .clone();
    c.reply(Ok(json!({"ok":true})));
    call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":retry_copy}),
    )
    .await;
    assert_eq!(c.calls()[1]["params"]["file"], path);
    assert!(!bytes.is_empty());
    c.reply(Ok(json!({"ok":true})));
    call(
        &a,
        &c,
        "index",
        "extension:invoke-action",
        json!({"actionId":"generate-save","formValues":f}),
    )
    .await;
    let saved = std::fs::read_dir(root.join("Downloads"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(std::fs::read(saved).unwrap(), bytes);
    assert_eq!(mock.calls.lock().unwrap().len(), 4);
}
