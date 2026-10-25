pub mod domain;
pub mod files;
pub mod shortener;
mod ui;
use domain::{Draft, Encoded};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tezbar_extension_sdk::{
    BoxFuture, CommandConcurrency, CoreResult, Extension, ExtensionContext,
};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

pub const ID: &str = "com.tezbar.native.qrcode-generator";
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Receipt {
    #[default]
    None,
    Pending,
    Failed,
    Ack,
}
#[derive(Debug)]
pub(crate) struct Prepared {
    id: String,
    text: String,
    format: String,
    encoded: Encoded,
    save: Receipt,
    path: Option<PathBuf>,
    open: Receipt,
    copy: Receipt,
    clipboard: Option<files::OwnedFile>,
}
pub(crate) struct Session {
    command: String,
    draft: Draft,
    revision: u64,
    preferences: Value,
    page: bool,
    issue: String,
    attempted: Option<Draft>,
    generation_retry: Option<(String, String)>,
    prepared: Option<Prepared>,
}
impl Session {
    fn new(command: &str, prefs: Value, format: String) -> Self {
        Self {
            command: command.into(),
            draft: Draft::new(domain::preference(prefs["defaultColor"].as_str()), format),
            revision: 0,
            preferences: prefs,
            page: false,
            issue: String::new(),
            attempted: None,
            generation_retry: None,
            prepared: None,
        }
    }
}
type Sessions = Mutex<HashMap<String, Arc<AsyncMutex<Session>>>>;
pub struct Adapter {
    sessions: Sessions,
    network: Arc<dyn shortener::Transport>,
    downloads: Option<PathBuf>,
}
impl Adapter {
    pub fn production() -> Result<Self, String> {
        Ok(Self::with_services(
            Arc::new(shortener::Network::new()?),
            None,
        ))
    }
    /// Test seam: explicit disposable Downloads and mock transport; never selected through wire payloads.
    pub fn with_services(
        network: Arc<dyn shortener::Transport>,
        downloads: Option<PathBuf>,
    ) -> Self {
        Self {
            sessions: Mutex::default(),
            network,
            downloads,
        }
    }
    fn downloads(&self) -> Result<PathBuf, String> {
        self.downloads
            .clone()
            .or_else(dirs::download_dir)
            .ok_or_else(|| "Platform Downloads directory is unavailable".into())
    }
    async fn run(
        &self,
        ctx: &ExtensionContext,
        command: &str,
        operation: &str,
        payload: &Value,
    ) -> CoreResult {
        let sid = payload["sessionId"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 256)
            .ok_or("Invalid session")?;
        if operation == "extension:dispose-session" {
            self.sessions.lock().unwrap().remove(sid);
            return Ok(json!({}));
        }
        let session = {
            let mut map = self.sessions.lock().unwrap();
            if operation == "extension:run-command" {
                if map.contains_key(sid) {
                    return Err("Session already opened".into());
                }
                if map.len() >= 128 {
                    return Err("Session limit reached".into());
                }
                let source_preferences = &payload["preferences"];
                let prefs = json!({
                    "defaultColor": domain::preference(source_preferences["defaultColor"].as_str()),
                    "primaryAction": source_preferences["primaryAction"].as_str().filter(|v|["save","copy","inline"].contains(v)),
                });
                // Actual V1 host does not implement storeValue. No defaultFormat preference binding.
                let session =
                    Arc::new(AsyncMutex::new(Session::new(command, prefs, String::new())));
                map.insert(sid.into(), session.clone());
                session
            } else {
                map.get(sid).cloned().ok_or("Session no longer exists")?
            }
        };
        let mut s = session.lock().await;
        if s.command != command {
            return Err("Command/session mismatch".into());
        }
        if operation == "extension:run-command" {
            if command != "index" {
                // Reserve acquisition before await: a cancelled launch never reacquires on refresh.
                s.issue =
                    "Input acquisition was interrupted; reopen command to acquire again".into();
                let method = if command == "clipboard" {
                    "host.clipboard.read"
                } else {
                    "host.selection.read"
                };
                let result = ctx.host(method, json!({})).await;
                let text = match result {
                    Ok(Value::String(text)) => text,
                    _ => {
                        s.issue = "Could not read source text".into();
                        return Ok(ui::render(&s));
                    }
                };
                if domain::js_trim(&text).is_empty() {
                    s.issue = if command == "clipboard" {
                        "No text found in clipboard"
                    } else {
                        "You need to select an URL to generate the qrcode"
                    }
                    .into();
                    return Ok(ui::render(&s));
                }
                s.draft.url = text;
                s.draft.format = "png-bg".into();
                s.draft.validate()?;
                self.generate(ctx, &mut s, "generate-show").await?;
                s.page = true;
            }
            return Ok(ui::render(&s));
        }
        if matches!(
            operation,
            "extension:refresh-session" | "extension:search-text-changed" | "extension:load-more"
        ) {
            return Ok(ui::render(&s));
        }
        if operation != "extension:invoke-action" {
            return Err("Unsupported operation".into());
        }
        let action = payload["actionId"].as_str().ok_or("Missing action ID")?;
        if action == "edit" || action == "__nav_pop__" {
            s.page = false;
            return Ok(ui::render(&s));
        }
        if action.starts_with("field:") {
            let fields = payload["formValues"]
                .as_object()
                .ok_or("Field change requires current draft")?;
            let rev = fields
                .get("__tezbarDraftRevision")
                .and_then(Value::as_u64)
                .ok_or("Renderer draft revision prerequisite missing")?;
            if rev <= s.revision {
                return Ok(ui::render(&s));
            }
            Self::merge(&mut s, fields)?;
            s.revision = rev;
            s.page = false;
            return Ok(ui::render(&s));
        }
        let retry = action.strip_prefix("retry-generation:");
        let retry_action = if let Some(token) = retry {
            Some(
                s.generation_retry
                    .as_ref()
                    .filter(|(id, _)| id == token)
                    .ok_or("Expired generation retry ignored")?
                    .1
                    .clone(),
            )
        } else {
            None
        };
        let action = retry_action.as_deref().unwrap_or(action);
        if command == "index"
            && ["generate-show", "generate-save", "generate-copy"].contains(&action)
        {
            let fields = payload["formValues"]
                .as_object()
                .ok_or("Submit requires all current form fields")?;
            // Reject incomplete maps; never run effects using stale, partially submitted values.
            for key in ["url", "format", "color", "shorten", "utmEnabled"] {
                if !fields.contains_key(key) {
                    return Err(
                        "Submit requires complete current form fields (host prerequisite)".into(),
                    );
                }
            }
            if fields.get("color").is_some_and(|v| v == "custom")
                && !fields.contains_key("customColor")
            {
                return Err("Submit requires custom color".into());
            }
            if fields.get("utmEnabled").is_some_and(|v| v == true)
                && [
                    "utmSource",
                    "utmMedium",
                    "utmCampaign",
                    "utmTerm",
                    "utmContent",
                ]
                .iter()
                .any(|k| !fields.contains_key(*k))
            {
                return Err("Submit requires current tracking fields".into());
            }
            let rev = fields
                .get("__tezbarDraftRevision")
                .and_then(Value::as_u64)
                .ok_or("Renderer draft revision prerequisite missing")?;
            if rev < s.revision {
                return Err("Stale form submission ignored".into());
            }
            Self::merge(&mut s, fields)?;
            s.revision = rev;
            if let Some(token) = retry {
                if s.generation_retry
                    .as_ref()
                    .is_none_or(|(id, _)| id != token)
                {
                    return Err("Generation retry draft changed; submit current fields".into());
                }
            }
            if let Err(error) = s.draft.validate() {
                s.issue = error;
                return Ok(ui::render(&s));
            }
            let same = s.attempted.as_ref() == Some(&s.draft);
            if !same || retry.is_some() {
                self.generate(ctx, &mut s, action).await?;
            }
            if s.prepared.is_none() {
                return Ok(ui::render(&s));
            }
            s.issue.clear();
            match action {
                "generate-show" => s.page = true,
                "generate-save" => self.effect(ctx, &mut s, "save", false).await?,
                _ => self.effect(ctx, &mut s, "copy", false).await?,
            }
            return Ok(ui::render(&s));
        }
        let (effect, token) = action.split_once(':').ok_or("Unknown action")?;
        if s.prepared.as_ref().is_none_or(|p| p.id != token) {
            return Err("Expired output action ignored".into());
        }
        match effect {
            "save" | "copy" => self.effect(ctx, &mut s, effect, false).await?,
            "retry-save" => self.effect(ctx, &mut s, "save", true).await?,
            "retry-copy" => self.effect(ctx, &mut s, "copy", true).await?,
            "retry-open" => self.effect(ctx, &mut s, "open", true).await?,
            _ => return Err("Unknown output action".into()),
        }
        Ok(ui::render(&s))
    }
    fn merge(s: &mut Session, fields: &serde_json::Map<String, Value>) -> Result<(), String> {
        let mut value = serde_json::to_value(&s.draft).unwrap();
        let object = value.as_object_mut().unwrap();
        for (k, v) in fields {
            if object.contains_key(k) {
                object.insert(k.clone(), v.clone());
            }
        }
        let draft: Draft = serde_json::from_value(value).map_err(|_| "Invalid form field type")?;
        let bytes = serde_json::to_vec(&draft).unwrap().len();
        if bytes > 32_768 || draft.url.len() > domain::MAX_INPUT {
            return Err("Form exceeds resource limits; nothing was truncated".into());
        }
        if s.draft != draft {
            s.prepared = None;
            s.attempted = None;
            s.generation_retry = None;
            s.issue.clear();
        }
        s.draft = draft;
        Ok(())
    }
    async fn generate(
        &self,
        ctx: &ExtensionContext,
        s: &mut Session,
        action: &str,
    ) -> Result<(), String> {
        // Consume recovery intent before awaiting. A dropped future never reissues it.
        s.generation_retry = None;
        s.attempted = Some(s.draft.clone());
        s.prepared = None;
        s.issue =
            "Generation was interrupted. Edit input or reopen to request another generation".into();
        let mut text = s.draft.prepare();
        if s.draft.shorten && domain::http(&text).is_some() {
            text = tokio::select! {biased;_ = ctx.cancelled()=>return Err("Operation cancelled".into()),result=shortener::shorten(&self.network,&text)=>match result {
                Ok(v)=>v,Err(e)=>{Self::generation_failed(s, action, e);return Ok(());}
            }};
        }
        let encoded = match domain::encode(
            Some(&text),
            &domain::preference(Some(s.draft.selected())),
            s.draft.format == "png-bg",
        ) {
            Ok(v) => v,
            Err(e) => {
                Self::generation_failed(s, action, e);
                return Ok(());
            }
        };
        s.prepared = Some(Prepared {
            id: Uuid::new_v4().to_string(),
            text,
            format: s.draft.format.clone(),
            encoded,
            save: Receipt::None,
            path: None,
            open: Receipt::None,
            copy: Receipt::None,
            clipboard: None,
        });
        s.issue.clear();
        Ok(())
    }
    fn generation_failed(s: &mut Session, action: &str, error: String) {
        if s.command == "index" {
            s.issue = format!("{error}. Use Retry Generation to try again");
            s.generation_retry = Some((Uuid::new_v4().to_string(), action.into()));
        } else {
            s.issue = error;
        }
    }
    async fn effect(
        &self,
        ctx: &ExtensionContext,
        s: &mut Session,
        effect: &str,
        retry: bool,
    ) -> Result<(), String> {
        let p = s.prepared.as_mut().ok_or("No prepared output")?;
        match effect {
            "save" => {
                if p.save == Receipt::Ack {
                    s.issue = "Already saved; file preserved".into();
                    return Ok(());
                }
                if p.save != Receipt::None && !retry {
                    s.issue = "Save outcome requires explicit Retry Save".into();
                    return Ok(());
                }
                p.save = Receipt::Pending;
                let result = (|| {
                    let name = domain::filename(&p.text, p.format == "svg")?;
                    files::save(
                        &self.downloads()?,
                        &name,
                        if p.format == "svg" {
                            &p.encoded.svg
                        } else {
                            &p.encoded.png
                        },
                    )
                })();
                match result {
                    Ok(path) => {
                        p.path = Some(path);
                        p.save = Receipt::Ack;
                        s.issue = "Saved to Downloads".into();
                    }
                    Err(e) => {
                        p.save = Receipt::Failed;
                        s.issue = e;
                        return Ok(());
                    }
                }
                if s.command == "index" {
                    self.open(ctx, p, &mut s.issue).await;
                }
            }
            "open" => {
                if p.save != Receipt::Ack {
                    return Err("No acknowledged saved file".into());
                }
                self.open(ctx, p, &mut s.issue).await;
            }
            "copy" => {
                if p.format == "svg" {
                    s.issue="Failed to copy SVG: file clipboard is unavailable on this host. Save the SVG, or edit and choose PNG to copy image pixels".into();
                    return Ok(());
                }
                if p.copy == Receipt::Ack {
                    s.issue = "Already copied; clipboard was not changed again".into();
                    return Ok(());
                }
                if p.copy != Receipt::None && !retry {
                    s.issue="Clipboard outcome is unknown or failed. Explicit Retry Copy uses the same prepared PNG".into();
                    return Ok(());
                }
                if ![
                    "host.clipboard.writeImage",
                    "clipboard.writeImage",
                    "clipboard",
                ]
                .iter()
                .any(|cap| ctx.granted_capabilities.contains(*cap))
                {
                    s.issue = "Failed to copy PNG: clipboard capability was not granted".into();
                    p.copy = Receipt::Failed;
                    return Ok(());
                }
                let dir = ctx.paths.data_dir.join("clipboard-staging");
                if p.clipboard.is_none() {
                    p.clipboard = Some(files::stage_clipboard(&dir, &p.encoded.png)?);
                }
                let file = p.clipboard.as_mut().unwrap();
                // Host may have consumed pixels even if cancellation discards SDK staged state.
                // Preserve the file until an explicit positive image-write acknowledgement.
                file.retain();
                p.copy = Receipt::Pending;
                s.issue =
                    "Clipboard outcome is unknown; Retry Copy uses the same prepared PNG".into();
                let path = file
                    .path()
                    .to_str()
                    .ok_or("Non-Unicode clipboard path")?
                    .to_owned();
                match ctx
                    .host("host.clipboard.writeImage", json!({"file":path}))
                    .await
                {
                    Ok(v) if acknowledgement(&v) => {
                        p.copy = Receipt::Ack;
                        file.cleanup_after_ack();
                        p.clipboard = None;
                        s.issue = "QR Code copied to clipboard".into();
                    }
                    _ => {
                        p.copy = Receipt::Failed;
                        s.issue="Clipboard write failed or was not acknowledged. Retry Copy uses the same PNG".into();
                    }
                }
            }
            _ => return Err("Unsupported effect".into()),
        }
        Ok(())
    }
    async fn open(&self, ctx: &ExtensionContext, p: &mut Prepared, issue: &mut String) {
        if p.open == Receipt::Ack {
            *issue = "Saved file already opened".into();
            return;
        }
        p.open = Receipt::Pending;
        *issue = "File saved; opening was not acknowledged. Retry Open will not rewrite it".into();
        let Some(path) = p.path.as_ref().and_then(|p| p.to_str()) else {
            return;
        };
        match ctx.host("host.shell.open", json!({"target":path})).await {
            Ok(v) if acknowledgement(&v) => {
                p.open = Receipt::Ack;
                *issue = "QR Code saved and opened".into();
            }
            _ => {
                p.open = Receipt::Failed;
            }
        }
    }
}
/// Exact structured acknowledgements returned by the selected platform services.
pub fn acknowledgement(v: &Value) -> bool {
    let Some(object) = v.as_object() else {
        return false;
    };
    object.get("ok") == Some(&Value::Bool(true))
        && object.keys().all(|key| key == "ok" || key == "message")
        && object.get("message").is_none_or(Value::is_string)
}

impl Extension for Adapter {
    fn id(&self) -> &'static str {
        ID
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    fn commands(&self) -> &'static [&'static str] {
        &["index", "clipboard", "selection"]
    }
    fn concurrency(&self, _: &str) -> CommandConcurrency {
        CommandConcurrency::Serialize
    }
    fn handle<'a>(
        &'a self,
        ctx: &'a ExtensionContext,
        command: &'a str,
        operation: &'a str,
        payload: &'a Value,
        _state: &'a mut Value,
    ) -> BoxFuture<'a, CoreResult> {
        Box::pin(self.run(ctx, command, operation, payload))
    }
    fn shutdown<'a>(&'a self, _: &'a ExtensionContext) -> BoxFuture<'a, CoreResult> {
        Box::pin(async move {
            self.sessions.lock().unwrap().clear();
            Ok(json!({}))
        })
    }
}
