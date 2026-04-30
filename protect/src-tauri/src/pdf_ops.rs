use base64::Engine;
use pdfium::{PdfiumBitmapFormat, PdfiumDocument, PdfiumRenderFlags};
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Serialize)]
pub struct PdfInfo {
    pub path: String,
    pub size: String,
    pub size_bytes: u64,
    pub pages: u32,
}

pub struct PdfSession {
    pub doc: PdfiumDocument,
    pub info: PdfInfo,
}

fn init_pdfium() -> Result<(), String> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    if let Some(ref dir) = exe_dir {
        pdfium::set_library_location(dir.to_string_lossy().as_ref());
    }
    if pdfium::try_lib().is_err() {
        pdfium::set_library_location("./");
        pdfium::try_lib().map_err(|e| format!("load pdfium: {e}"))?;
    }
    Ok(())
}

pub fn open_pdf(path: &str) -> Result<PdfSession, String> {
    let p = Path::new(path);
    if !p.is_file() {
        return Err(format!("file not found: {path}"));
    }
    let size_bytes = p.metadata().map(|m| m.len()).unwrap_or(0);
    init_pdfium()?;
    let doc = PdfiumDocument::new_from_path(path, None).map_err(|e| format!("open pdf: {e}"))?;
    let pages = doc.page_count() as u32;
    Ok(PdfSession {
        info: PdfInfo {
            path: path.to_string(),
            size: format_size(size_bytes),
            size_bytes,
            pages,
        },
        doc,
    })
}

pub fn render_page(session: &PdfSession, page_idx: u32, dpi: u32, output: &str) -> Result<(), String> {
    let page = session
        .doc
        .page(page_idx as i32)
        .map_err(|e| format!("get page {page_idx}: {e}"))?;

    let bounds = page
        .boundaries()
        .default()
        .map_err(|e| format!("bounds: {e}"))?;
    let width_pts = bounds.width();
    let width_px = (width_pts / 72.0 * dpi as f32) as i32;

    let bitmap = page
        .render_at_width(width_px, PdfiumBitmapFormat::Bgra, PdfiumRenderFlags::empty())
        .map_err(|e| format!("render: {e}"))?;

    bitmap
        .save(output, image::ImageFormat::Png)
        .map_err(|e| format!("save png: {e}"))?;

    Ok(())
}

pub fn render_to_base64(session: &PdfSession, page_idx: u32, dpi: u32) -> Result<String, String> {
    let page = session
        .doc
        .page(page_idx as i32)
        .map_err(|e| format!("get page {page_idx}: {e}"))?;

    let bounds = page
        .boundaries()
        .default()
        .map_err(|e| format!("bounds: {e}"))?;
    let width_pts = bounds.width();
    let width_px = (width_pts / 72.0 * dpi as f32) as i32;

    let bitmap = page
        .render_at_width(width_px, PdfiumBitmapFormat::Bgra, PdfiumRenderFlags::empty())
        .map_err(|e| format!("render: {e}"))?;

    let img = bitmap
        .as_rgba8_image()
        .map_err(|e| format!("convert: {e}"))?;

    let mut buf = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut buf),
        image::ImageFormat::Png,
    )
    .map_err(|e| format!("encode: {e}"))?;

    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&buf)
    ))
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn parse_page_range(text: &str, max_pages: u32) -> Result<Vec<u32>, String> {
    let mut pages = std::collections::BTreeSet::new();
    for part in text.split(&[',', '\u{ff0c}'][..]) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(pos) = part.find('-') {
            let a: u32 = part[..pos]
                .trim()
                .parse()
                .map_err(|_| format!("invalid: {part}"))?;
            let b: u32 = part[pos + 1..]
                .trim()
                .parse()
                .map_err(|_| format!("invalid: {part}"))?;
            for p in a..=b {
                if p >= 1 && p <= max_pages {
                    pages.insert(p - 1);
                }
            }
        } else {
            let p: u32 = part
                .trim()
                .parse()
                .map_err(|_| format!("invalid: {part}"))?;
            if p >= 1 && p <= max_pages {
                pages.insert(p - 1);
            }
        }
    }
    if pages.is_empty() {
        return Err("empty range".into());
    }
    Ok(pages.into_iter().collect())
}
