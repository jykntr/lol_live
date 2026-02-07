use crate::types::CsRegion;
use image::{GrayImage, RgbaImage};
use xcap::Monitor;

/// Find the primary monitor and return its dimensions and a captured image.
pub fn capture_screen() -> Result<(u32, u32, RgbaImage), String> {
    let monitors = Monitor::all().map_err(|e| format!("Failed to list monitors: {e}"))?;
    let monitor = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| Monitor::all().ok()?.into_iter().next())
        .ok_or_else(|| "No monitors found".to_string())?;

    let width = monitor
        .width()
        .map_err(|e| format!("Failed to get width: {e}"))?;
    let height = monitor
        .height()
        .map_err(|e| format!("Failed to get height: {e}"))?;

    let img = monitor
        .capture_image()
        .map_err(|e| format!("Screen capture failed: {e}"))?;

    Ok((width, height, img))
}

/// Crop the CS region from a full screenshot and convert to grayscale.
pub fn crop_cs_region(img: &RgbaImage, region: &CsRegion) -> GrayImage {
    let cropped =
        image::imageops::crop_imm(img, region.x, region.y, region.width, region.height).to_image();

    let gray = image::DynamicImage::ImageRgba8(cropped).into_luma8();
    gray
}
