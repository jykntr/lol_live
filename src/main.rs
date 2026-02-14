use clap::Parser;
use lol_live::{api, capture, config, detect, ocr, types};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::watch;
use types::ApiData;

#[derive(Parser)]
#[command(about = "Real-time League of Legends CS tracker")]
struct Args {
    /// Enable debug mode (saves diagnostic screenshots and OCR output)
    #[arg(long)]
    debug: bool,

    /// Record screenshots for building an OCR test suite, optionally to a directory
    #[arg(long, num_args = 0..=1, default_missing_value = ".")]
    record: Option<PathBuf>,

    /// Path to League of Legends game.cfg config file
    #[arg(short = 'c', long = "game-config")]
    game_config: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let debug = args.debug;
    let record_dir = args.record;
    if debug {
        eprintln!("[OCR] Debug mode enabled");
    }
    if let Some(ref dir) = record_dir {
        eprintln!(
            "[record] Recording mode enabled, saving to {}",
            dir.display()
        );
        if !dir.exists() {
            std::fs::create_dir_all(dir).expect("Failed to create record directory");
        }
    }

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("Failed to build HTTP client");

    // Channels
    let (api_tx, api_rx) = watch::channel::<Option<ApiData>>(None);
    let (ocr_tx, ocr_rx) = watch::channel::<Option<i64>>(None);

    // Load League config (HUD scale, resolution, etc.)
    let league_config = config::load(args.game_config.as_ref());
    if debug {
        eprintln!("[OCR] League config: {league_config}");
    }
    let hud_scale = league_config.hud_scale;
    let window_mode = league_config.window_mode;

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
    let ocr_api_rx = api_rx.clone();
    let ocr_handle = if ocr_available {
        Some(tokio::task::spawn_blocking(move || {
            let mut lt = match ocr::init_tesseract() {
                Some(lt) => lt,
                None => return,
            };

            let mut game_active = false;

            loop {
                // Wait for game to start
                if !game_active {
                    eprintln!("[OCR] Waiting for game to start...");
                    loop {
                        if ocr_api_rx
                            .borrow()
                            .as_ref()
                            .is_some_and(|d| d.game_time > 0.0)
                        {
                            break;
                        }
                        std::thread::sleep(Duration::from_secs(1));
                    }
                    eprintln!("[OCR] Game detected, starting OCR captures");
                    game_active = true;
                }

                // Do an initial capture to detect resolution and CS region
                let (screen_w, screen_h) = match capture::capture_screen() {
                    Ok((w, h, _)) => {
                        eprintln!("[OCR] Screen resolution: {}x{}", w, h);
                        (w, h)
                    }
                    Err(e) => {
                        eprintln!("[OCR] Initial screen capture failed: {e}");
                        std::thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                };

                let region = detect::detect_cs_region(screen_w, screen_h, hud_scale);
                eprintln!(
                    "[OCR] CS region: x={}, y={}, w={}, h={}",
                    region.x, region.y, region.width, region.height
                );

                const DEBUG_SCREENSHOT_MAX: u64 = 100;
                let mut screenshot_index: u64 = 0;
                let mut last_ocr_ok = true;

                // Record mode state
                let mut last_recorded_cs: Option<i64> = None;
                let mut record_fail_index: u64 = 0;

                // Capture loop — runs while game is active
                loop {
                    // Check if game ended or not yet started (gameTime <= 0)
                    if !ocr_api_rx
                        .borrow()
                        .as_ref()
                        .is_some_and(|d| d.game_time > 0.0)
                    {
                        eprintln!("[OCR] Game ended or not active, pausing OCR captures");
                        game_active = false;
                        break;
                    }

                    match capture::capture_screen() {
                        Ok((_, _, img)) => {
                            let gray = capture::crop_cs_region(&img, &region);
                            let cs = ocr::read_cs(&mut lt, &gray, debug);
                            if let Some(cs_val) = cs {
                                if detect::is_valid_cs(cs_val) {
                                    let _ = ocr_tx.send(Some(cs_val));

                                    // Record: save on first successful parse of a new CS value
                                    if let Some(ref dir) = record_dir
                                        && (last_recorded_cs.is_none()
                                            || cs_val > last_recorded_cs.unwrap())
                                    {
                                        let path = dir.join(format!("record_res_{screen_w}x{screen_h}_wm_{window_mode}_cs_{cs_val}.png"));
                                        if let Err(e) = img.save(&path) {
                                            eprintln!(
                                                "[record] Failed to save {}: {e}",
                                                path.display()
                                            );
                                        } else {
                                            eprintln!("[record] Saved {}", path.display());
                                        }
                                        last_recorded_cs = Some(cs_val);
                                    }
                                }
                                last_ocr_ok = true;
                            } else {
                                if debug && last_ocr_ok && screenshot_index < DEBUG_SCREENSHOT_MAX {
                                    let base = format!(
                                        "debug_screenshot_res_{screen_w}x{screen_h}_wm_{window_mode}_{screenshot_index}"
                                    );
                                    let annotated = format!("{base}.png");
                                    capture::save_screenshot(&img, &annotated, &region);
                                    eprintln!("[OCR] Saved screenshot to {annotated}");
                                    let raw = format!("{base}_raw.png");
                                    if let Err(e) = img.save(&raw) {
                                        eprintln!(
                                            "[OCR] Failed to save raw screenshot to {raw}: {e}"
                                        );
                                    } else {
                                        eprintln!("[OCR] Saved raw screenshot to {raw}");
                                    }
                                    screenshot_index += 1;
                                }

                                // Record: save on first failure after a success
                                if let Some(ref dir) = record_dir
                                    && last_ocr_ok
                                {
                                    let path = dir.join(format!("record_res_{screen_w}x{screen_h}_wm_{window_mode}_fail_{record_fail_index}.png"));
                                    if let Err(e) = img.save(&path) {
                                        eprintln!(
                                            "[record] Failed to save {}: {e}",
                                            path.display()
                                        );
                                    } else {
                                        eprintln!("[record] Saved {}", path.display());
                                    }
                                    record_fail_index += 1;
                                }

                                last_ocr_ok = false;
                            }
                        }
                        Err(e) => {
                            eprintln!("[OCR] Screen capture failed: {e}");
                        }
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
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
