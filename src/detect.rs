use crate::types::CsRegion;

/// Calculate the CS counter region based on screen dimensions and HUD scale.
///
/// At 1920x1080 with default HUD scale, the CS counter is roughly at
/// (screen_width - 138, 5) with size (30, 20). We scale proportionally
/// for other resolutions.
pub fn detect_cs_region(screen_width: u32, screen_height: u32, hud_scale: f64) -> CsRegion {
    let base_width: f64 = 1920.0;
    let base_height: f64 = 1080.0;

    let scale_x = (screen_width as f64 / base_width) * hud_scale;
    let scale_y = (screen_height as f64 / base_height) * hud_scale;

    let x = screen_width as f64 - (138.0 * scale_x);
    let y = 5.0 * scale_y;
    let width = 30.0 * scale_x;
    let height = 20.0 * scale_y;

    CsRegion {
        x: x as u32,
        y: y as u32,
        width: width as u32,
        height: height as u32,
    }
}

/// Validate that a parsed CS value is plausible (0-999).
pub fn is_valid_cs(cs: i64) -> bool {
    (0..=999).contains(&cs)
}
