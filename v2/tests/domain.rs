use serde_json::Value;
use std::fs;
use tezbar_qrcode_generator::{domain::*, files};
fn decode(bytes: &[u8]) -> String {
    let image = image::load_from_memory(bytes).unwrap().to_rgba8();
    let gray = image::GrayImage::from_fn(image.width(), image.height(), |x, y| {
        let p = image.get_pixel(x, y).0;
        let alpha = p[3] as u32;
        let lum = (p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000;
        image::Luma([((lum * alpha + 255 * (255 - alpha)) / 255) as u8])
    });
    let gray = image::imageops::resize(&gray, 2048, 2048, image::imageops::FilterType::Nearest);
    let mut prepared = rqrr::PreparedImage::prepare(gray);
    let grids = prepared.detect_grids();
    assert_eq!(grids.len(), 1);
    let (meta, text) = grids[0].decode().unwrap();
    assert_eq!(meta.ecc_level, 0); // rqrr's format bits: M=00.
    text
}
#[test]
fn independent_decoder_roundtrips_and_capacity() {
    for text in [
        "Hello, world!".into(),
        "https://example.com/a?x=%20&y=中#fragment".into(),
        "Қазақша 🙂\nUnicode e\u{301}".into(),
        "0123456789".repeat(100),
        "ABCD 123 $%*+-./:".repeat(35),
        "abcdefg".repeat(100),
        "1".repeat(5596),
        "A".repeat(3391),
        "a".repeat(2331),
    ] {
        eprintln!("roundtrip {} bytes", text.len());
        let qr = encode(Some(&text), "#000000", true).unwrap();
        assert_eq!(decode(&qr.png), text);
        assert!(qr.size <= 177);
        assert_eq!(qr.matrix.len(), qr.size * qr.size);
    }
    for text in [
        "1".repeat(5597),
        "A".repeat(3392),
        "a".repeat(2332),
        "x".repeat(MAX_INPUT + 1),
    ] {
        assert!(encode(Some(&text), "#000000", true).is_err());
    }
    assert!(encode(None, "#000000", false)
        .unwrap_err()
        .contains("undefined"));
    assert!(encode(Some(""), "#000000", false).is_err());
}
#[test]
fn dimensions_palette_alpha_quiet_zone_svg() {
    for color in ["#1D8348", "0A66C2", "#fff", "AbC"] {
        for white in [false, true] {
            let qr = encode(Some("https://example.com"), color, white).unwrap();
            let png = image::load_from_memory(&qr.png).unwrap().to_rgba8();
            assert_eq!(png.dimensions(), (512, 512));
            let hex = normalize(color).unwrap();
            let dark = [
                u8::from_str_radix(&hex[1..3], 16).unwrap(),
                u8::from_str_radix(&hex[3..5], 16).unwrap(),
                u8::from_str_radix(&hex[5..7], 16).unwrap(),
                255,
            ];
            let light = if white {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 0]
            };
            for (x, y, p) in png.enumerate_pixels() {
                assert!(p.0 == dark || p.0 == light);
                if x < 512 * 4 / (qr.size as u32 + 8) || y < 512 * 4 / (qr.size as u32 + 8) {
                    assert_eq!(p.0, light);
                }
            }
            let svg = String::from_utf8(qr.svg).unwrap();
            assert!(svg.contains("width=\"1536\" height=\"1536\""));
            assert!(svg.contains(&format!("viewBox=\"0 0 {} {}\"", qr.size + 8, qr.size + 8)));
            assert!(!svg.contains("https://example.com"));
            assert!(!svg.contains("<rect"));
        }
    }
    let qr = encode(Some("white-backed"), "#1D8348", false).unwrap();
    assert_eq!(decode(&qr.png), "white-backed");
}
#[test]
fn source_oracle_colors_urls_names() {
    let oracle: Value = serde_json::from_str(include_str!("../qa/fixtures/oracle.json")).unwrap();
    for v in oracle["colors"].as_array().unwrap() {
        let raw = v["raw"].as_str().unwrap();
        assert_eq!(preference(Some(raw)), v["resolved"]);
        assert_eq!(low_contrast(raw), v["low"]);
    }
    for v in oracle["urls"].as_array().unwrap() {
        let d: Draft = serde_json::from_value(v["draft"].clone()).unwrap();
        assert_eq!(d.prepare(), v["result"], "{}", d.url);
    }
    for v in oracle["names"].as_array().unwrap() {
        let name = filename(v["text"].as_str().unwrap(), false);
        if v["result"].is_null() {
            assert!(name.is_err());
        } else {
            assert_eq!(name.unwrap(), v["result"]);
        }
    }
}
#[test]
fn save_atomic_collision_symlink_permissions_and_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let path = files::save(&root, "qrcode-test.png", b"one").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"one");
    assert!(files::save(&root, "qrcode-test.png", b"two").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"one");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    assert!(files::save(&root, "../qrcode-other.png", b"bad").is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::{symlink, PermissionsExt};
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        symlink(&path, root.join("qrcode-link.png")).unwrap();
        assert!(files::save(&root, "qrcode-link.png", b"bad").is_err());
        let dir = root.join("linked");
        symlink(&root, &dir).unwrap();
        assert!(files::save(&dir, "qrcode-no.png", b"bad").is_err());
        let owned = files::OwnedFile::create(&root, "tmp", b"staged").unwrap();
        let old = owned.path().to_owned();
        fs::rename(&old, root.join("qrcode-published.png")).unwrap();
        fs::write(&old, b"staged").unwrap();
        drop(owned);
        assert_eq!(fs::read(&old).unwrap(), b"staged");
    }
}
#[test]
fn finite_matrix_pixel_oracle_comparison_and_svg_exports() {
    let source: Value = serde_json::from_str(include_str!("../qa/fixtures/oracle.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".work/qr-output");
    fs::create_dir_all(&root).unwrap();
    let mut rows = Vec::new();
    for (i, v) in source["qr"].as_array().unwrap().iter().enumerate() {
        let text = v["text"].as_str().unwrap();
        let qr = encode(Some(text), "#1D8348", false).unwrap();
        let same = qr.size == v["size"].as_u64().unwrap() as usize
            && qr
                .matrix
                .iter()
                .zip(v["matrix"].as_array().unwrap())
                .all(|(a, b)| *a == (b.as_u64() == Some(1)));
        let pixels = image::load_from_memory(&qr.png).unwrap().to_rgba8();
        let hash = files::digest(pixels.as_raw());
        if same {
            assert_eq!(
                hash, v["pixelSha256"],
                "same matrix must render same pixels"
            );
        }
        assert_eq!(decode(&qr.png), text);
        fs::write(root.join(format!("{i}.svg")), &qr.svg).unwrap();
        fs::write(root.join(format!("{i}.png")), &qr.png).unwrap();
        rows.push(serde_json::json!({"index":i,"rustSize":qr.size,"sourceSize":v["size"],"sameMatrix":same,"samePixels":hash==v["pixelSha256"],"pngSha256":files::digest(&qr.png),"svgSha256":files::digest(&qr.svg)}));
    }
    fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}

#[test]
fn independently_rasterized_svg_roundtrips_if_present() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".work/qr-output");
    let source: Value = serde_json::from_str(include_str!("../qa/fixtures/oracle.json")).unwrap();
    if !root.join("0-svg.png").exists() {
        return;
    }
    for (i, v) in source["qr"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            decode(&fs::read(root.join(format!("{i}-svg.png"))).unwrap()),
            v["text"].as_str().unwrap()
        );
    }
}
#[test]
fn filenames_unicode_control_and_transport_policy() {
    assert!(filename("https://a.test/a\nhttp://b.test/b", false)
        .unwrap()
        .contains("a.test,http---b.test"));
    assert!(filename("user@host:999/path", false)
        .unwrap()
        .starts_with("qrcode-user@host"));
    assert_eq!(
        filename("a<>\"\\|?*\u{1}b", true).unwrap(),
        "qrcode-a--------b.svg"
    );
    let text = format!("{}🙂", "a".repeat(79));
    let name = filename(&text, false).unwrap();
    assert!(!name.contains('🙂'));
    assert!(!name.contains('\u{fffd}'));
    assert!(serde_json::from_str::<Value>(r#""\ud800""#).is_err());
}

#[test]
fn clipboard_staging_concurrent_count_and_incoming_byte_bound() {
    use std::sync::{Arc, Barrier};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    for i in 0..127 {
        fs::write(root.join(format!("unknown-{i}")), b"existing").unwrap();
    }
    let start = Arc::new(Barrier::new(16));
    let workers: Vec<_> = (0..16)
        .map(|_| {
            let root = root.clone();
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                files::stage_clipboard(&root, b"new PNG")
            })
        })
        .collect();
    let staged: Vec<_> = workers
        .into_iter()
        .filter_map(|worker| worker.join().unwrap().ok())
        .collect();
    assert_eq!(staged.len(), 1);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 128);
    for i in 0..127 {
        assert_eq!(
            fs::read(root.join(format!("unknown-{i}"))).unwrap(),
            b"existing"
        );
    }
    drop(staged);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 127);

    let bytes_dir = root.join("byte-limit");
    fs::create_dir(&bytes_dir).unwrap();
    let unknown = bytes_dir.join("unacknowledged");
    let file = fs::File::create(&unknown).unwrap();
    let limit = 256 * 1024 * 1024;
    // Sparse file: test the aggregate boundary without allocating/writing 256 MiB.
    file.set_len(limit - 4).unwrap();
    assert!(files::stage_clipboard(&bytes_dir, b"12345").is_err());
    assert_eq!(fs::read_dir(&bytes_dir).unwrap().count(), 1);
    let staged = files::stage_clipboard(&bytes_dir, b"1234").unwrap();
    assert_eq!(fs::metadata(&unknown).unwrap().len(), limit - 4);
    assert!(files::stage_clipboard(&bytes_dir, b"x").is_err());
    assert_eq!(fs::read_dir(&bytes_dir).unwrap().count(), 2);
    drop(staged);
    assert_eq!(fs::read_dir(&bytes_dir).unwrap().count(), 1);
}
