use clap::Parser;
use leptess::LepTess;
use lol_live::{api, capture, detect, ocr, types};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::watch;
use types::{ApiData, CsRegion};

#[derive(Parser)]
#[command(about = "Real-time League of Legends CS tracker")]
struct Args {
    /// Enable debug mode (saves diagnostic screenshots and OCR output)
    #[arg(long)]
    debug: bool,

    /// Record screenshots for building an OCR test suite, optionally to a directory
    #[arg(long, num_args = 0..=1, default_missing_value = ".")]
    record: Option<PathBuf>,
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

    let (api_tx, api_rx) = watch::channel::<Option<ApiData>>(None);
    let (ocr_tx, ocr_rx) = watch::channel::<Option<i64>>(None);

    let api_handle = tokio::spawn(async move {
        loop {
            let _ = api_tx.send(api::poll_game_data(&client).await);
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });

    let ocr_api_rx = api_rx.clone();
    let ocr_handle = if ocr::init_tesseract().is_some() {
        eprintln!("[OCR] Tesseract initialized successfully");
        Some(tokio::task::spawn_blocking(move || {
            run_ocr_loop(ocr_api_rx, ocr_tx, debug, record_dir);
        }))
    } else {
        eprintln!("[OCR] Tesseract not available, running in API-only mode");
        None
    };

    let display_handle = tokio::spawn(run_display_loop(api_rx, ocr_rx));

    let _ = api_handle.await;
    if let Some(h) = ocr_handle {
        let _ = h.await;
    }
    let _ = display_handle.await;
}

fn wait_for_game(api_rx: &watch::Receiver<Option<ApiData>>) {
    loop {
        if api_rx.borrow().as_ref().is_some_and(|d| d.game_time > 0.0) {
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn get_screen_resolution() -> (u32, u32) {
    loop {
        match capture::capture_screen() {
            Ok((w, h, _)) => {
                eprintln!("[OCR] Screen resolution: {}x{}", w, h);
                return (w, h);
            }
            Err(e) => {
                eprintln!("[OCR] Initial screen capture failed: {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn run_ocr_loop(
    api_rx: watch::Receiver<Option<ApiData>>,
    ocr_tx: watch::Sender<Option<i64>>,
    debug: bool,
    record_dir: Option<PathBuf>,
) {
    let Some(mut lt) = ocr::init_tesseract() else {
        return;
    };

    loop {
        eprintln!("[OCR] Waiting for game to start...");
        wait_for_game(&api_rx);
        eprintln!("[OCR] Game detected, starting OCR captures");

        let (screen_w, screen_h) = get_screen_resolution();
        let region = detect::detect_cs_region(screen_w, screen_h);
        eprintln!(
            "[OCR] CS region: x={}, y={}, w={}, h={}",
            region.x, region.y, region.width, region.height
        );

        run_capture_loop(
            &api_rx,
            &ocr_tx,
            &region,
            screen_w,
            screen_h,
            debug,
            &record_dir,
            &mut lt,
        );
    }
}

fn run_capture_loop(
    api_rx: &watch::Receiver<Option<ApiData>>,
    ocr_tx: &watch::Sender<Option<i64>>,
    region: &CsRegion,
    screen_w: u32,
    screen_h: u32,
    debug: bool,
    record_dir: &Option<PathBuf>,
    lt: &mut LepTess,
) {
    const DEBUG_SCREENSHOT_MAX: u64 = 100;
    let mut screenshot_index: u64 = 0;
    let mut last_ocr_ok = true;
    let mut last_recorded_cs: Option<i64> = None;
    let mut record_fail_index: u64 = 0;

    loop {
        if !api_rx.borrow().as_ref().is_some_and(|d| d.game_time > 0.0) {
            eprintln!("[OCR] Game ended or not active, pausing OCR captures");
            let _ = ocr_tx.send(None);
            break;
        }

        let img = match capture::capture_screen() {
            Ok((_, _, img)) => img,
            Err(e) => {
                eprintln!("[OCR] Screen capture failed: {e}");
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        let gray = capture::crop_cs_region(&img, region);
        let cs = ocr::read_cs(lt, &gray, debug);

        match cs.filter(|&v| detect::is_valid_cs(v)) {
            Some(cs_val) => {
                let _ = ocr_tx.send(Some(cs_val));
                if let Some(dir) = record_dir
                    && (last_recorded_cs.is_none() || cs_val > last_recorded_cs.unwrap())
                {
                    let path =
                        dir.join(format!("record_res_{screen_w}x{screen_h}_cs_{cs_val}.png"));
                    save_record(img.save(&path), &path);
                    last_recorded_cs = Some(cs_val);
                }
                last_ocr_ok = true;
            }
            None => {
                if debug && last_ocr_ok && screenshot_index < DEBUG_SCREENSHOT_MAX {
                    let base =
                        format!("debug_screenshot_res_{screen_w}x{screen_h}_{screenshot_index}");
                    let annotated = format!("{base}.png");
                    capture::save_screenshot(&img, &annotated, region);
                    eprintln!("[OCR] Saved screenshot to {annotated}");
                    let raw = format!("{base}_raw.png");
                    match img.save(&raw) {
                        Ok(_) => eprintln!("[OCR] Saved raw screenshot to {raw}"),
                        Err(e) => eprintln!("[OCR] Failed to save raw screenshot to {raw}: {e}"),
                    }
                    screenshot_index += 1;
                }
                if let Some(dir) = record_dir
                    && last_ocr_ok
                {
                    let path = dir.join(format!(
                        "record_res_{screen_w}x{screen_h}_fail_{record_fail_index}.png"
                    ));
                    save_record(img.save(&path), &path);
                    record_fail_index += 1;
                }
                last_ocr_ok = false;
            }
        }

        std::thread::sleep(Duration::from_secs(1));
    }
}

async fn run_display_loop(
    api_rx: watch::Receiver<Option<ApiData>>,
    ocr_rx: watch::Receiver<Option<i64>>,
) {
    let mut last_cs = -1i64;
    let mut last_print = tokio::time::Instant::now();
    let mut no_game_printed = false;

    loop {
        let api_data = api_rx.borrow().clone();
        let ocr_cs = *ocr_rx.borrow();

        match api_data {
            Some(data) => {
                let (cs, source) = match ocr_cs {
                    Some(cs) if detect::is_valid_cs(cs) => (cs, "OCR"),
                    _ => (data.cs, "API"),
                };

                let cs_changed = cs > last_cs;
                let elapsed = last_print.elapsed() >= Duration::from_secs(10);

                if cs_changed || elapsed {
                    last_cs = cs;
                    last_print = tokio::time::Instant::now();
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
                if last_cs != -1 || !no_game_printed {
                    println!("No live game detected...");
                    no_game_printed = true;
                    last_cs = -1;
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn save_record(result: Result<(), impl std::fmt::Display>, path: &std::path::Path) {
    if let Err(e) = result {
        eprintln!("[record] Failed to save {}: {e}", path.display());
    } else {
        eprintln!("[record] Saved {}", path.display());
    }
}
