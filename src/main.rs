mod api;
mod capture;
mod detect;
mod ocr;
mod types;

use std::time::Duration;
use tokio::sync::watch;
use types::ApiData;

#[tokio::main]
async fn main() {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("Failed to build HTTP client");

    // Channels
    let (api_tx, api_rx) = watch::channel::<Option<ApiData>>(None);
    let (ocr_tx, ocr_rx) = watch::channel::<Option<i64>>(None);

    // Try to init OCR
    let ocr_available = match ocr::init_tesseract() {
        Some(_) => {
            eprintln!("[OCR] Tesseract initialized successfully");
            true
        }
        None => {
            eprintln!("[OCR] Tesseract not available, running in API-only mode");
            false
        }
    };

    // API loop (3s)
    let api_handle = tokio::spawn(async move {
        loop {
            let data = api::poll_game_data(&client).await;
            let _ = api_tx.send(data);
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });

    // OCR loop (1s) - only if Tesseract is available
    let ocr_handle = if ocr_available {
        Some(tokio::task::spawn_blocking(move || {
            let mut lt = match ocr::init_tesseract() {
                Some(lt) => lt,
                None => return,
            };

            // Do an initial capture to detect resolution and CS region
            let (screen_w, screen_h) = match capture::capture_screen() {
                Ok((w, h, _)) => {
                    eprintln!("[OCR] Screen resolution: {}x{}", w, h);
                    (w, h)
                }
                Err(e) => {
                    eprintln!("[OCR] Initial screen capture failed: {e}");
                    return;
                }
            };

            let hud_scale = 1.0;
            let region = detect::detect_cs_region(screen_w, screen_h, hud_scale);
            eprintln!(
                "[OCR] CS region: x={}, y={}, w={}, h={}",
                region.x, region.y, region.width, region.height
            );

            loop {
                match capture::capture_screen() {
                    Ok((_, _, img)) => {
                        let gray = capture::crop_cs_region(&img, &region);
                        let cs = ocr::read_cs(&mut lt, &gray);
                        if let Some(cs_val) = cs {
                            if detect::is_valid_cs(cs_val) {
                                let _ = ocr_tx.send(Some(cs_val));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[OCR] Screen capture failed: {e}");
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }))
    } else {
        None
    };

    // Display loop (500ms)
    let display_handle = tokio::spawn(async move {
        let mut last_cs = -1i64;

        loop {
            let api_data = api_rx.borrow().clone();
            let ocr_cs = *ocr_rx.borrow();

            match api_data {
                Some(data) => {
                    // Prefer OCR CS when available, fall back to API CS
                    let (cs, source) = match ocr_cs {
                        Some(cs) if detect::is_valid_cs(cs) => (cs, "OCR"),
                        _ => (data.cs, "API"),
                    };

                    if cs > last_cs {
                        last_cs = cs;
                        let minutes = data.game_time / 60.0;
                        let cs_per_minute = if minutes > 0.0 {
                            cs as f64 / minutes
                        } else {
                            0.0
                        };
                        let game_mins = data.game_time as i64 / 60;
                        let game_secs = data.game_time as i64 % 60;
                        println!(
                            "[{source}] CS per minute: {cs_per_minute:.1} ({cs} CS @ {game_mins}:{game_secs:0>2})"
                        );
                    }
                }
                None => {
                    if last_cs != -1 {
                        last_cs = -1;
                        println!("No live game detected...");
                    } else {
                        // Only print once when no game is running
                        static PRINTED: std::sync::atomic::AtomicBool =
                            std::sync::atomic::AtomicBool::new(false);
                        if !PRINTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                            println!("No live game detected...");
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

    // Wait for all tasks (they run forever)
    let _ = api_handle.await;
    if let Some(h) = ocr_handle {
        let _ = h.await;
    }
    let _ = display_handle.await;
}
