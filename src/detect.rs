use crate::types::CsRegion;

/// Calculate the CS counter region based on screen dimensions.
///
/// The CS counter sits in League's top scoreboard, which scales with game
/// resolution but NOT with the HUD GlobalScale setting. At 1920x1080 the
/// CS
/// number text starts roughly 143 pixels from the right edge at y=3.
/// The region is sized to capture up to 3-digit CS values.
pub fn detect_cs_region(screen_width: u32, screen_height: u32) -> CsRegion {
    let base_height: f64 = 1080.0;

    // League's scoreboard scales uniformly based on screen height, not
    // independently per axis.  For 16:9 resolutions the two scale factors
    // are identical, but for non-16:9 (e.g. 16:10 at 2560×1600) using a
    // single height-derived scale keeps the crop aligned with the actual
    // glyph positions.
    let scale = screen_height as f64 / base_height;

    let x = screen_width as f64 - (143.0 * scale);
    let y = 3.0 * scale;
    let width = 50.0 * scale;
    let height = 20.0 * scale;

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
