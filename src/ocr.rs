use image::GrayImage;
use leptess::{LepTess, Variable};
use std::io::Cursor;
use std::path::PathBuf;

const ENG_TRAINEDDATA: &[u8] = include_bytes!("../tessdata/eng.traineddata");

/// Thresholds to try in order. 128 works for most digits; lower values
/// capture anti-aliased edges of rounder glyphs like "0" and "9".
const THRESHOLDS: &[u8] = &[128, 100, 60];

/// Write the embedded traineddata to a temp directory and return its path.
fn prepare_tessdata() -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("lol_live_tessdata");
    std::fs::create_dir_all(&dir).ok()?;
    let file_path = dir.join("eng.traineddata");
    if !file_path.exists() {
        std::fs::write(&file_path, ENG_TRAINEDDATA).ok()?;
    }
    Some(dir)
}

/// Try to initialize Tesseract. Returns None if initialization fails.
pub fn init_tesseract() -> Option<LepTess> {
    let tessdata_dir = prepare_tessdata()?;
    let mut lt = LepTess::new(Some(tessdata_dir.to_str()?), "eng").ok()?;
    let _ = lt.set_variable(Variable::TesseditCharWhitelist, "0123456789");
    let _ = lt.set_variable(Variable::TesseditPagesegMode, "7");
    let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let _ = lt.set_variable(Variable::DebugFile, null_device);
    Some(lt)
}

/// Trim whitespace around dark content, keeping a small margin.
fn trim_whitespace(img: &GrayImage, margin: u32) -> GrayImage {
    let (w, h) = (img.width(), img.height());
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;

    for (x, y, pixel) in img.enumerate_pixels() {
        if pixel.0[0] < 128 {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    // No dark pixels found — return original
    if max_x < min_x {
        return img.clone();
    }

    let x = min_x.saturating_sub(margin);
    let y = min_y.saturating_sub(margin);
    let crop_w = (max_x - min_x + 1 + margin * 2).min(w - x);
    let crop_h = (max_y - min_y + 1 + margin * 2).min(h - y);

    image::imageops::crop_imm(img, x, y, crop_w, crop_h).to_image()
}

/// Preprocess a grayscale image for OCR: binary threshold, trim, and upscale.
fn preprocess(img: &GrayImage, threshold: u8) -> GrayImage {
    let mut processed = img.clone();

    // Binary threshold + invert: light text on dark background becomes
    // dark text on white background, which Tesseract recognizes more reliably.
    for pixel in processed.pixels_mut() {
        pixel.0[0] = if pixel.0[0] > threshold { 0 } else { 255 };
    }

    // Trim surrounding whitespace so single digits aren't lost in a wide frame
    processed = trim_whitespace(&processed, 5);

    // Upscale 3x if text height is small
    if processed.height() < 30 {
        processed = image::imageops::resize(
            &processed,
            processed.width() * 3,
            processed.height() * 3,
            image::imageops::FilterType::Lanczos3,
        );
    }

    processed
}

/// Attempt OCR at a single threshold. Returns the parsed CS value if successful.
fn try_ocr(lt: &mut LepTess, img: &GrayImage, threshold: u8, debug: bool) -> Option<i64> {
    let processed = preprocess(img, threshold);

    let mut buf = Vec::new();
    processed
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .ok()?;

    lt.set_image_from_mem(&buf).ok()?;

    let confidence = lt.mean_text_conf();
    let text = lt.get_utf8_text().ok()?;
    let trimmed = text.trim();

    if debug {
        eprintln!(
            "[OCR] threshold={threshold}: text={:?} | conf={confidence}",
            trimmed
        );
    }

    if trimmed.is_empty() {
        return None;
    }

    trimmed.parse::<i64>().ok()
}

/// Run OCR on a grayscale image and return the parsed CS value.
/// Tries multiple thresholds to handle varying glyph anti-aliasing.
/// When `debug` is true, logs OCR internals to stderr.
pub fn read_cs(lt: &mut LepTess, img: &GrayImage, debug: bool) -> Option<i64> {
    for &threshold in THRESHOLDS {
        if let Some(cs) = try_ocr(lt, img, threshold, debug) {
            return Some(cs);
        }
    }

    if debug {
        eprintln!("[OCR] All thresholds failed");
        if let Err(e) = img.save("debug_cs_crop.png") {
            eprintln!("[OCR] Failed to save debug_cs_crop.png: {e}");
        }
        if let Err(e) = preprocess(img, THRESHOLDS[0]).save("debug_cs_preprocessed.png") {
            eprintln!("[OCR] Failed to save debug_cs_preprocessed.png: {e}");
        }
    }

    None
}
