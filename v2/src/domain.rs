//! Pure, bounded adaptations of own V1 config/url/utils behavior.
use qrcode::{Color, EcLevel, QrCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use url::Url;

pub const MAX_INPUT: usize = 16_384;
pub const PRESETS: [(&str, &str); 6] = [
    ("Black", "#000000"),
    ("Blue", "#0A66C2"),
    ("Green", "#1D8348"),
    ("Red", "#C0392B"),
    ("Purple", "#6C3483"),
    ("Orange", "#D35400"),
];
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'))
}
pub fn normalize(s: &str) -> Option<String> {
    let s = js_trim(s).strip_prefix('#').unwrap_or(js_trim(s));
    if ![3, 6].contains(&s.len()) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!(
        "#{}",
        if s.len() == 3 {
            s.chars().flat_map(|c| [c, c]).collect()
        } else {
            s.to_owned()
        }
    ))
}
pub fn preference(s: Option<&str>) -> String {
    s.and_then(normalize).unwrap_or_else(|| "#000000".into())
}
pub fn low_contrast(s: &str) -> bool {
    let Some(s) = normalize(s) else {
        return false;
    };
    let channel = |i| {
        let c = u8::from_str_radix(&s[i..i + 2], 16).unwrap() as f64 / 255.;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5) > 0.4
}
pub fn http(s: &str) -> Option<Url> {
    Url::parse(js_trim(s))
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Draft {
    pub url: String,
    pub format: String,
    pub color: String,
    pub custom_color: String,
    pub shorten: bool,
    pub utm_enabled: bool,
    pub utm_source: String,
    pub utm_medium: String,
    pub utm_campaign: String,
    pub utm_term: String,
    pub utm_content: String,
}
impl Draft {
    pub fn new(color: String, format: String) -> Self {
        Self {
            url: String::new(),
            format,
            color: "custom".into(),
            custom_color: color,
            shorten: false,
            utm_enabled: false,
            utm_source: String::new(),
            utm_medium: String::new(),
            utm_campaign: String::new(),
            utm_term: String::new(),
            utm_content: String::new(),
        }
    }
    pub fn selected(&self) -> &str {
        if self.color == "custom" {
            &self.custom_color
        } else {
            &self.color
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if js_trim(&self.url).is_empty() {
            return Err("URL or content is required".into());
        }
        if self.url.len() > MAX_INPUT {
            return Err("Content exceeds 16384 UTF-8 bytes; nothing was truncated".into());
        }
        if !["png", "png-bg", "svg"].contains(&self.format.as_str()) {
            return Err("Choose a format".into());
        }
        if self.color == "custom" && normalize(self.selected()).is_none() {
            return Err("Enter a valid hex color, e.g. #1D8348 or 1D8348".into());
        }
        if [
            &self.utm_source,
            &self.utm_medium,
            &self.utm_campaign,
            &self.utm_term,
            &self.utm_content,
        ]
        .iter()
        .any(|s| s.len() > 2048)
        {
            return Err("Tracking parameter exceeds 2048 bytes".into());
        }
        Ok(())
    }
    pub fn prepare(&self) -> String {
        if !self.utm_enabled {
            return self.url.clone();
        }
        let Some(mut u) = http(&self.url) else {
            return self.url.clone();
        };
        // URLSearchParams.set replaces first matching pair and deletes subsequent duplicates,
        // retaining its first position. Only serialize the query if at least one set occurs.
        let mut pairs: Vec<(String, String)> = u
            .query_pairs()
            .map(|(a, b)| (a.into_owned(), b.into_owned()))
            .collect();
        let mut changed = false;
        for (key, raw) in [
            ("utm_source", &self.utm_source),
            ("utm_medium", &self.utm_medium),
            ("utm_campaign", &self.utm_campaign),
            ("utm_term", &self.utm_term),
            ("utm_content", &self.utm_content),
        ] {
            let value = js_trim(raw);
            if value.is_empty() {
                continue;
            }
            changed = true;
            let first = pairs.iter().position(|(k, _)| k == key);
            if let Some(i) = first {
                pairs[i].1 = value.into();
                let mut pos = 0;
                pairs.retain(|(k, _)| {
                    let keep = k != key || pos == i;
                    pos += 1;
                    keep
                });
            } else {
                pairs.push((key.into(), value.into()));
            }
        }
        if changed {
            u.set_query(None);
            u.query_pairs_mut().extend_pairs(pairs);
        }
        u.into()
    }
}
#[derive(Clone, Debug)]
pub struct Encoded {
    pub png: Vec<u8>,
    pub svg: Vec<u8>,
    pub matrix: Vec<bool>,
    pub size: usize,
}
pub fn encode(text: Option<&str>, color: &str, white: bool) -> Result<Encoded, String> {
    let text = text.ok_or("URL is undefined")?;
    if text.is_empty() {
        return Err("Cannot encode empty content".into());
    }
    if text.len() > MAX_INPUT {
        return Err("QR content exceeds 16384 UTF-8 bytes; nothing was truncated".into());
    }
    let hex = normalize(color).ok_or("Invalid color")?;
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
        .map_err(|_| "QR capacity exceeded; nothing was truncated")?;
    let size = code.width();
    let matrix: Vec<bool> = code.to_colors().iter().map(|c| *c == Color::Dark).collect();
    let rgb: Vec<u8> = (1..7)
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let mut pixels = vec![0u8; 512 * 512 * 4];
    // node-qrcode renderer: nearest-neighbour floor in scaled (size + 8) square.
    for y in 0..512 {
        for x in 0..512 {
            let mx = x * (size + 8) / 512;
            let my = y * (size + 8) / 512;
            let dark = mx >= 4
                && my >= 4
                && mx < size + 4
                && my < size + 4
                && matrix[(my - 4) * size + mx - 4];
            let p = &mut pixels[(y * 512 + x) * 4..(y * 512 + x) * 4 + 4];
            if dark {
                p.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            } else if white {
                p.copy_from_slice(&[255, 255, 255, 255]);
            }
        }
    }
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, 512, 512);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|_| "PNG header failed")?;
        writer
            .write_image_data(&pixels)
            .map_err(|_| "PNG encoding failed")?;
    }
    // No user text is interpolated into markup; only numeric module positions and validated hex.
    let extent = size + 8;
    let mut svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1536\" height=\"1536\" viewBox=\"0 0 {extent} {extent}\" shape-rendering=\"crispEdges\"><path fill=\"{hex}\" d=\"");
    for y in 0..size {
        for x in 0..size {
            if matrix[y * size + x] {
                svg.push_str(&format!("M{} {}h1v1h-1z", x + 4, y + 4));
            }
        }
    }
    svg.push_str("\"/></svg>");
    Ok(Encoded {
        png,
        svg: svg.into_bytes(),
        matrix,
        size,
    })
}
/// JS full-match array stringification, then scheme stripping and UTF-16 slice.
/// If slice cuts a surrogate pair, drop that final unit rather than create an invalid path.
pub fn filename(text: &str, svg: bool) -> Result<String, String> {
    static MATCH: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(?:https?://)?(?:[^@/\n]+@)?(?:www\.)?([^:/\n]+)").unwrap());
    let mut matches = Vec::new();
    let mut end = 0;
    let starts = std::iter::once(0).chain(text.char_indices().filter_map(|(i, c)| {
        matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}').then_some(i + c.len_utf8())
    }));
    for start in starts {
        if start < end {
            continue;
        }
        if let Some(m) = MATCH.find(&text[start..]) {
            end = start + m.end();
            matches.push(m.as_str());
        }
    }
    if matches.is_empty() {
        return Err("Invalid URL format for filename".into());
    }
    let joined = matches.join(",");
    let raw: String = joined
        .split_inclusive(['\n', '\r', '\u{2028}', '\u{2029}'])
        .map(|line| {
            line.strip_prefix("https://")
                .or_else(|| line.strip_prefix("http://"))
                .unwrap_or(line)
        })
        .collect();
    let clean: String = raw
        .chars()
        .map(|c| {
            if c <= '\u{1f}' || "<>:\"/\\|?*".contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let mut units: Vec<u16> = clean.encode_utf16().take(80).collect();
    if units.last().is_some_and(|u| (0xd800..=0xdbff).contains(u)) {
        units.pop();
    }
    let value = String::from_utf16(&units).map_err(|_| "Invalid Unicode filename")?;
    // Prefix prevents reserved Windows device names; remove terminal dot/space for Windows.
    let value = value.trim_end_matches(['.', ' ']);
    Ok(format!(
        "qrcode-{}.{}",
        if value.is_empty() { "qrcode" } else { value },
        if svg { "svg" } else { "png" }
    ))
}
