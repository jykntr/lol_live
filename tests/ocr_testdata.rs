use std::path::Path;

use image::ImageReader;
use lol_live::{capture, detect, ocr};

/// Parse a testdata directory name like "res_2560x1440_wm_0" into (width, height, window_mode).
fn parse_dir_name(name: &str) -> Option<(u32, u32, u32)> {
    // Expected format: res_WWWWxHHHH_wm_Y
    let rest = name.strip_prefix("res_")?;
    let (res_part, wm_part) = rest.split_once("_wm_")?;
    let (w, h) = res_part.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?, wm_part.parse().ok()?))
}

#[test]
fn test_ocr_against_testdata() {
    let testdata_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    if !testdata_dir.exists() {
        eprintln!("testdata/ directory not found, skipping OCR test");
        return;
    }

    let mut lt = ocr::init_tesseract().expect("Failed to initialize Tesseract");

    let mut total = 0;
    let mut passed = 0;
    let mut failures: Vec<String> = Vec::new();

    for entry in std::fs::read_dir(&testdata_dir).expect("Failed to read testdata/") {
        let entry = entry.expect("Failed to read directory entry");
        if !entry.file_type().unwrap().is_dir() {
            continue;
        }

        let dir_name = entry.file_name().to_string_lossy().to_string();
        let (screen_w, screen_h, _wm) = match parse_dir_name(&dir_name) {
            Some(v) => v,
            None => {
                eprintln!("Skipping unrecognized directory: {dir_name}");
                continue;
            }
        };

        let region = detect::detect_cs_region(screen_w, screen_h, 1.0);

        for img_entry in std::fs::read_dir(entry.path()).expect("Failed to read image directory") {
            let img_entry = img_entry.expect("Failed to read image entry");
            let img_path = img_entry.path();

            if img_path.extension().and_then(|e| e.to_str()) != Some("png") {
                continue;
            }

            let expected_cs: i64 = match img_path.file_stem().and_then(|s| s.to_str()) {
                Some(stem) => match stem.parse() {
                    Ok(v) => v,
                    Err(_) => {
                        eprintln!("Skipping non-numeric filename: {}", img_path.display());
                        continue;
                    }
                },
                None => continue,
            };

            let img = ImageReader::open(&img_path)
                .unwrap_or_else(|e| panic!("Failed to open {}: {e}", img_path.display()))
                .decode()
                .unwrap_or_else(|e| panic!("Failed to decode {}: {e}", img_path.display()))
                .into_rgba8();

            let gray = capture::crop_cs_region(&img, &region);
            let actual_cs = ocr::read_cs(&mut lt, &gray, false);

            total += 1;
            if actual_cs == Some(expected_cs) {
                passed += 1;
            } else {
                let msg = format!(
                    "{dir_name}/{}: expected {expected_cs}, got {:?}",
                    img_path.file_name().unwrap().to_string_lossy(),
                    actual_cs,
                );
                eprintln!("FAIL: {msg}");
                failures.push(msg);
            }
        }
    }

    eprintln!("\nOCR test results: {passed}/{total} passed");

    if !failures.is_empty() {
        panic!(
            "{} of {total} OCR tests failed:\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}
