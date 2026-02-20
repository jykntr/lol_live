use std::path::Path;

use image::ImageReader;
use lol_live::{capture, detect, ocr};

/// Parse a testdata directory name like "res_2560x1440" into (width, height).
fn parse_dir_name(name: &str) -> Option<(u32, u32)> {
    let rest = name.strip_prefix("res_")?;
    let (w, h) = rest.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

fn run_ocr_test(dir_name: &str) {
    let testdata_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(dir_name);
    if !testdata_dir.exists() {
        eprintln!("testdata/{dir_name} directory not found, skipping");
        return;
    }

    let (screen_w, screen_h) =
        parse_dir_name(dir_name).unwrap_or_else(|| panic!("Bad directory name: {dir_name}"));

    let region = detect::detect_cs_region(screen_w, screen_h);
    let mut lt = ocr::init_tesseract().expect("Failed to initialize Tesseract");

    let mut total = 0;
    let mut passed = 0;
    let mut failures: Vec<String> = Vec::new();

    for img_entry in std::fs::read_dir(&testdata_dir).expect("Failed to read image directory") {
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

    eprintln!("{dir_name}: {passed}/{total} passed");

    if !failures.is_empty() {
        panic!(
            "{} of {total} OCR tests failed:\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

#[test]
fn test_ocr_res_2560x1440() {
    run_ocr_test("res_2560x1440");
}

#[test]
fn test_ocr_res_3840x2160() {
    run_ocr_test("res_3840x2160");
}
