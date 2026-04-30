#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod logger;
mod pdf_ops;
mod printer;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;

struct CancelFlag(Arc<AtomicBool>);

#[tauri::command]
fn open_pdf(path: String) -> Result<pdf_ops::PdfInfo, String> {
    let session = pdf_ops::open_pdf(&path)?;
    Ok(session.info)
}

#[tauri::command]
fn browse_pdf() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("PDF 文件", &["pdf"])
        .pick_file()
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn get_printers() -> Vec<String> {
    printer::list_printers()
}

#[tauri::command]
fn get_default_printer() -> Option<String> {
    printer::default_printer()
}

#[tauri::command]
fn parse_range(text: String, max_pages: u32) -> Result<Vec<u32>, String> {
    pdf_ops::parse_page_range(&text, max_pages)
}

#[tauri::command]
fn get_history() -> Vec<logger::LogRecord> {
    logger::get_records(20)
}

#[tauri::command]
fn preview_page(path: String, page_num: u32, preview_dpi: u32) -> Result<String, String> {
    let session = pdf_ops::open_pdf(&path)?;
    pdf_ops::render_to_base64(&session, page_num, preview_dpi)
}

#[tauri::command]
fn clear_history() {
    logger::clear();
}

#[tauri::command]
fn cancel_print(flag: tauri::State<CancelFlag>) {
    flag.0.store(true, Ordering::SeqCst);
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct AppSettings {
    printer: Option<String>,
    dpi: u32,
    copies: u32,
    paper_size: i16,
    orientation: i16,
    color_mode: i16,
    duplex: i16,
    collate: bool,
}

fn settings_path() -> std::path::PathBuf {
    std::env::temp_dir().join("big_pdf_printer").join("settings.json")
}

#[tauri::command]
fn load_settings() -> AppSettings {
    let p = settings_path();
    if let Ok(s) = std::fs::read_to_string(&p) {
        if let Ok(s) = serde_json::from_str(&s) {
            return s;
        }
    }
    AppSettings {
        printer: None,
        dpi: 200,
        copies: 1,
        paper_size: 9,
        orientation: 1,
        color_mode: 2,
        duplex: 1,
        collate: true,
    }
}

#[tauri::command]
fn save_settings(s: AppSettings) {
    let p = settings_path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    serde_json::to_string_pretty(&s)
        .ok()
        .and_then(|s| std::fs::write(&p, s).ok());
}

#[tauri::command]
fn start_print(
    app: tauri::AppHandle,
    flag: tauri::State<CancelFlag>,
    path: String,
    printer_name: String,
    pages: Vec<u32>,
    dpi: u32,
    copies: u32,
    paper_size: i16,
    orientation: i16,
    color_mode: i16,
    duplex: i16,
    collate: bool,
) {
    flag.0.store(false, Ordering::SeqCst);
    let cancel = flag.0.clone();
    let file = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let total = pages.len() as u32 * copies;

    let settings = printer::PrintSettings {
        paper_size,
        orientation,
        color_mode,
        duplex,
        collate,
        copies,
    };

    std::thread::spawn(move || {
        let result = run_print(app.clone(), cancel, &path, &printer_name, &pages, dpi, &settings);
        match result {
            Ok((done, failed, elapsed)) => {
                let status = if failed > 0 { "部分失败" } else { "完成" };
                let _ = app.emit(
                    "progress",
                    serde_json::json!({
                        "type": "done",
                        "current": done,
                        "total": total,
                        "failed": failed,
                        "status": format!("{status} · {done}/{total} 页 · 耗时 {elapsed:.0}秒"),
                    }),
                );
                logger::add(&file, done, status, &format!("{elapsed:.0}秒"));
            }
            Err(e) => {
                let _ = app.emit(
                    "progress",
                    serde_json::json!({
                        "type": "error",
                        "status": e,
                    }),
                );
            }
        }
    });
}

fn run_print(
    app: tauri::AppHandle,
    cancel: Arc<AtomicBool>,
    path: &str,
    printer_name: &str,
    pages: &[u32],
    dpi: u32,
    settings: &printer::PrintSettings,
) -> Result<(u32, u32, f64), String> {
    let session = pdf_ops::open_pdf(path)?;
    let temp_dir = std::env::temp_dir().join("big_pdf_printer");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("无法创建临时目录: {e}"))?;

    let total = pages.len() as u32 * settings.copies;
    let mut done = 0u32;
    let mut failed = 0u32;
    let t0 = std::time::Instant::now();

    for _ in 0..settings.copies {
        // 每份拷贝创建一个打印任务
        let hdc = match printer::create_printer_dc(printer_name, settings) {
            Ok(h) => h,
            Err(e) => {
                let elapsed = t0.elapsed().as_secs_f64();
                logger::add(
                    &session.info.path,
                    done,
                    "失败",
                    &format!("{elapsed:.0}秒"),
                );
                let _ = std::fs::remove_dir_all(&temp_dir);
                return Err(format!("创建打印机 DC 失败: {e}"));
            }
        };

        if let Err(e) = printer::start_doc(hdc) {
            printer::delete_dc(hdc);
            let elapsed = t0.elapsed().as_secs_f64();
            logger::add(&session.info.path, done, "失败", &format!("{elapsed:.0}秒"));
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(format!("StartDoc 失败: {e}"));
        }

        for &pi in pages {
            if cancel.load(Ordering::Relaxed) {
                printer::abort_doc(hdc);
                printer::delete_dc(hdc);
                let elapsed = t0.elapsed().as_secs_f64();
                let _ = app.emit(
                    "progress",
                    serde_json::json!({
                        "type": "progress",
                        "current": done,
                        "total": total,
                        "status": format!("已取消 · 完成 {done}/{total} 页"),
                    }),
                );
                logger::add(
                    &session.info.path,
                    done,
                    "已取消",
                    &format!("{elapsed:.0}秒"),
                );
                let _ = std::fs::remove_dir_all(&temp_dir);
                return Ok((done, failed, elapsed));
            }

            let pn = pi + 1;
            let _ = app.emit(
                "progress",
                serde_json::json!({
                    "type": "progress",
                    "current": done,
                    "total": total,
                    "status": format!("渲染第 {pn} 页..."),
                }),
            );

            let out_path = temp_dir.join(format!("page_{:06}.png", pi));
            let out_str = out_path.to_string_lossy().to_string();

            if let Err(e) = pdf_ops::render_page(&session, pi, dpi, &out_str) {
                failed += 1;
                done += 1;
                let _ = app.emit(
                    "progress",
                    serde_json::json!({
                        "type": "progress",
                        "current": done,
                        "total": total,
                        "status": format!("渲染第 {pn} 页失败: {e}"),
                    }),
                );
                continue;
            }

            let _ = app.emit(
                "progress",
                serde_json::json!({
                    "type": "progress",
                    "current": done,
                    "total": total,
                    "status": format!("打印第 {pn} 页..."),
                }),
            );

            if let Err(e) = printer::print_page(hdc, &out_str, settings.color_mode) {
                failed += 1;
                let _ = app.emit(
                    "progress",
                    serde_json::json!({
                        "type": "progress",
                        "current": done + 1,
                        "total": total,
                        "status": format!("打印第 {pn} 页失败: {e}"),
                    }),
                );
            }

            let _ = std::fs::remove_file(&out_str);
            done += 1;

            if done > 0 && done < total {
                let elapsed = t0.elapsed().as_secs_f64();
                let eta = elapsed / done as f64 * (total - done) as f64;
                let eta_str = if eta > 60.0 {
                    format!("剩余约 {} 分钟", (eta / 60.0) as u32)
                } else {
                    format!("剩余约 {} 秒", eta as u32)
                };
                let _ = app.emit(
                    "progress",
                    serde_json::json!({
                        "type": "progress",
                        "current": done,
                        "total": total,
                        "status": format!("第 {pn} 页完成  {}", eta_str),
                    }),
                );
            }

            if cancel.load(Ordering::Relaxed) {
                break;
            }
        }

        // 结束当前拷贝的打印任务
        if cancel.load(Ordering::Relaxed) {
            printer::abort_doc(hdc);
        } else if let Err(e) = printer::end_doc(hdc) {
            printer::delete_dc(hdc);
            let _ = app.emit(
                "progress",
                serde_json::json!({
                    "type": "error",
                    "status": format!("EndDoc 失败: {e}"),
                }),
            );
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(format!("EndDoc 失败: {e}"));
        }
        printer::delete_dc(hdc);

        if cancel.load(Ordering::Relaxed) {
            break;
        }
    }

    let elapsed = t0.elapsed().as_secs_f64();
    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok((done, failed, elapsed))
}

fn main() {
    tauri::Builder::default()
        .manage(CancelFlag(Arc::new(AtomicBool::new(false))))
        .invoke_handler(tauri::generate_handler![
            open_pdf,
            preview_page,
            browse_pdf,
            get_printers,
            get_default_printer,
            parse_range,
            get_history,
            clear_history,
            start_print,
            cancel_print,
            load_settings,
            save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("启动失败");
}
