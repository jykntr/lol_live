use image::GrayImage;
use leptess::{LepTess, Variable};
use std::io::Cursor;

/// Try to initialize Tesseract. Returns None if Tesseract is not installed.
pub fn init_tesseract() -> Option<LepTess> {
    let mut lt = LepTess::new(None, "eng").ok()?;
    let _ = lt.set_variable(Variable::TesseditCharWhitelist, "0123456789");
    let _ = lt.set_variable(Variable::TesseditPagesegMode, "7");
    Some(lt)
}

/// Preprocess a grayscale image for OCR: binary threshold + upscale if small.
fn preprocess(img: &GrayImage) -> GrayImage {
    let mut processed = img.clone();

    // Binary threshold at 128
    for pixel in processed.pixels_mut() {
        pixel.0[0] = if pixel.0[0] > 128 { 255 } else { 0 };
    }

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

/// Run OCR on a grayscale image and return the parsed CS value.
/// Returns None if confidence is too low or the result isn't a valid number.
pub fn read_cs(lt: &mut LepTess, img: &GrayImage) -> Option<i64> {
    let processed = preprocess(img);

    // Encode as PNG bytes
    let mut buf = Vec::new();
    processed
        .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .ok()?;

    lt.set_image_from_mem(&buf).ok()?;

    let confidence = lt.mean_text_conf();
    if confidence < 50 {
        return None;
    }

    let text = lt.get_utf8_text().ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    trimmed.parse::<i64>().ok()
}
