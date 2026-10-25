use crate::{domain, Receipt, Session};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
fn action(id: &str, title: &str, submit: bool) -> Value {
    json!({"id":id,"title":title,"kind":if submit{"submit-form"}else{"action"}})
}
fn node(kind: &str, props: Value) -> Value {
    json!({"type":kind,"props":props,"children":[]})
}
fn field(kind: &str, id: &str, title: &str, value: Value) -> Value {
    node(
        kind,
        json!({"id":id,"title":title,"value":value,"actionId":format!("field:{id}")}),
    )
}
pub(crate) fn render(s: &Session) -> Value {
    let p = s.prepared.as_ref();
    let mut result = if s.page || s.command != "index" {
        let mut props = json!({"markdown":"", "__tezbarDraftRevision":s.revision});
        if s.command == "index" {
            props["backActionId"] = json!("edit");
        }
        let mut actions = Vec::new();
        if let Some(p) = p {
            let (mime, data) = if p.format == "svg" {
                ("image/svg+xml", &p.encoded.svg)
            } else {
                ("image/png", &p.encoded.png)
            };
            // Only internally generated bitmap/vector bytes; never interpolate input as Markdown.
            // V1's query suffix after base64 corrupts image data on this renderer. Omit it.
            props["markdown"] = json!(format!(
                "![qrcode](data:{mime};base64,{})",
                STANDARD.encode(data)
            ));
            if s.command == "index" {
                actions.push(action("edit", "Edit QR Code", false));
            } else {
                actions.push(action(
                    &format!("save:{}", p.id),
                    "Save to Downloads",
                    false,
                ));
                actions.push(action(
                    &format!("copy:{}", p.id),
                    "Copy to Clipboard",
                    false,
                ));
                actions[0]["shortcut"] = json!({"modifiers":["cmd"],"key":"s"});
                actions[1]["shortcut"] = json!({"modifiers":["cmd"],"key":"c"});
            }
        }
        tezbar_extension_sdk::view("QR Code Generator", node("Detail", props), actions)
    } else {
        let d = &s.draft;
        let mut fields = vec![field(
            "Form.TextField",
            "url",
            "URL or Content",
            json!(d.url),
        )];
        let mut format = field("Form.Dropdown", "format", "Format", json!(d.format));
        format["props"]["data"] = json!([{"title":"PNG (Transparent)","value":"png"},{"title":"PNG (w/BG)","value":"png-bg"},{"title":"SVG","value":"svg"}]);
        fields.push(format);
        let mut color = field("Form.Dropdown", "color", "QR Color", json!(d.color));
        let mut presets: Vec<Value> = domain::PRESETS
            .iter()
            .map(|(title, value)| json!({"title":title,"value":value}))
            .collect();
        presets.push(json!({"title":"Custom…","value":"custom"}));
        color["props"]["data"] = json!(presets);
        if domain::low_contrast(d.selected()) {
            color["props"]["error"] = json!("Low contrast (warning; generation remains available)");
        }
        fields.push(color);
        if d.color == "custom" {
            fields.push(field(
                "Form.TextField",
                "customColor",
                "Custom Color (Hex)",
                json!(d.custom_color),
            ));
        }
        let mut shorten = field("Form.Checkbox", "shorten", "Shorten link", json!(d.shorten));
        shorten["props"]["info"]=json!("Sends the URL to a link shortener (is.gd, da.gd, then TinyURL) to create a short link.");
        fields.push(shorten);
        if d.shorten && !domain::js_trim(&d.url).is_empty() && domain::http(&d.url).is_none() {
            fields.push(node("Form.Description",json!({"text":"Shortening applies to http(s) links only — this content will be encoded as-is."})));
        }
        fields.push(node("Form.Separator", json!({})));
        fields.push(field(
            "Form.Checkbox",
            "utmEnabled",
            "Add tracking parameters (UTM)",
            json!(d.utm_enabled),
        ));
        if d.utm_enabled {
            fields.push(node("Form.Description",json!({"text":"UTM parameters are appended to http(s) URLs for campaign tracking."})));
            for (id, title, value) in [
                ("utmSource", "utm_source", &d.utm_source),
                ("utmMedium", "utm_medium", &d.utm_medium),
                ("utmCampaign", "utm_campaign", &d.utm_campaign),
                ("utmTerm", "utm_term", &d.utm_term),
                ("utmContent", "utm_content", &d.utm_content),
            ] {
                fields.push(field("Form.TextField", id, title, json!(value)));
            }
        }
        let order = match s.preferences["primaryAction"].as_str() {
            Some("save") => ["save", "show", "copy"],
            Some("copy") => ["copy", "save", "show"],
            _ => ["show", "save", "copy"],
        };
        let actions = order
            .iter()
            .map(|a| {
                action(
                    &format!("generate-{a}"),
                    match *a {
                        "save" => "Generate and Save",
                        "copy" => "Generate and Copy to Clipboard",
                        _ => "Generate and Show",
                    },
                    true,
                )
            })
            .collect();
        tezbar_extension_sdk::view(
            "QR Code Generator",
            json!({"type":"Form","props":{"__tezbarDraftRevision":s.revision},"children":fields}),
            actions,
        )
    };
    if let Some((token, _)) = &s.generation_retry {
        result["actions"].as_array_mut().unwrap().push(action(
            &format!("retry-generation:{token}"),
            "Retry Generation",
            true,
        ));
    }
    if let Some(p) = p {
        let actions = result["actions"].as_array_mut().unwrap();
        if matches!(p.save, Receipt::Pending | Receipt::Failed) {
            actions.push(action(
                &format!("retry-save:{}", p.id),
                "Retry Save Prepared Output",
                false,
            ));
        }
        if matches!(p.copy, Receipt::Pending | Receipt::Failed) {
            actions.push(action(
                &format!("retry-copy:{}", p.id),
                "Retry Copy Prepared PNG",
                false,
            ));
        }
        if p.save == Receipt::Ack && matches!(p.open, Receipt::Pending | Receipt::Failed) {
            actions.push(action(
                &format!("retry-open:{}", p.id),
                "Retry Open Saved File",
                false,
            ));
        }
    }
    result["refreshMs"] = Value::Null;
    if !s.issue.is_empty() {
        result["message"] = json!(s.issue);
    }
    result
}
