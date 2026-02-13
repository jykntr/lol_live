use crate::types::CsRegion;

/// Calculate the CS counter region based on screen dimensions.
///
/// The CS counter sits in League's top scoreboard, which scales with game
/// resolution but NOT with the HUD GlobalScale setting. At 1920x1080 the
/// CS
/// number text starts roughly 143 pixels from the right edge at y=3.
/// The region is sized to capture up to 3-digit CS values.
pub fn detect_cs_region(screen_width: u32, screen_height: u32, _hud_scale: f64) -> CsRegion {
    let base_width: f64 = 1920.0;
    let base_height: f64 = 1080.0;

    let scale_x = screen_width as f64 / base_width;
    let scale_y = screen_height as f64 / base_height;

    let x = screen_width as f64 - (143.0 * scale_x);
    let y = 3.0 * scale_y;
    let width = 50.0 * scale_x;
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
