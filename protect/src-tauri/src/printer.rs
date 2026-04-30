use image::GenericImageView;
use windows::core::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::Printing::*;
use windows::Win32::Storage::Xps::*;

// ── DEVMODE dmFields flags ──────────────────────────────
const DM_ORIENTATION: u32 = 0x00000001;
const DM_PAPERSIZE: u32   = 0x00000002;
const DM_COPIES: u32      = 0x00000100;
const DM_COLOR: u32       = 0x00000800;
const DM_DUPLEX: u32      = 0x00001000;
const DM_COLLATE: u32     = 0x00008000;

// ── paper sizes ────────────────────────────────────────
const DMPAPER_LETTER: i16 = 1;
const DMPAPER_LEGAL: i16  = 5;
const DMPAPER_A3: i16     = 8;
const DMPAPER_A4: i16     = 9;
const DMPAPER_A5: i16     = 11;
const DMPAPER_B5: i16     = 13;

// ── orientation ────────────────────────────────────────
const DMORIENT_PORTRAIT: i16  = 1;
const DMORIENT_LANDSCAPE: i16 = 2;

// ── color ──────────────────────────────────────────────
const DMCOLOR_MONOCHROME: i16 = 1;
const DMCOLOR_COLOR: i16       = 2;

// ── duplex ─────────────────────────────────────────────
const DMDUP_SIMPLEX: i16    = 1;
const DMDUP_VERTICAL: i16   = 2; // flip long edge
const DMDUP_HORIZONTAL: i16 = 3; // flip short edge

// ── collate ────────────────────────────────────────────
const DMCOLLATE_TRUE: i16  = 1;
const DMCOLLATE_FALSE: i16 = 0;

// ═══════════════════════════════════════════════════════
pub struct PrintSettings {
    pub paper_size: i16,
    pub orientation: i16,
    pub color_mode: i16,
    pub duplex: i16,
    pub collate: bool,
    pub copies: u32,
}

// ═══════════════════════════════════════════════════════

fn create_printer_dc(printer_name: &str, settings: &PrintSettings) -> std::result::Result<HDC, String> {
    let printer_wide: Vec<u16> = printer_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut dm: DEVMODEW = unsafe { std::mem::zeroed() };
    dm.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
    dm.dmFields = DEVMODE_FIELD_FLAGS(DM_ORIENTATION | DM_PAPERSIZE | DM_COPIES | DM_COLOR | DM_DUPLEX | DM_COLLATE);
    dm.Anonymous1.Anonymous1.dmOrientation = settings.orientation;
    dm.Anonymous1.Anonymous1.dmPaperSize = settings.paper_size;
    dm.Anonymous1.Anonymous1.dmCopies = settings.copies as i16;
    dm.dmColor = DEVMODE_COLOR(settings.color_mode);
    dm.dmDuplex = DEVMODE_DUPLEX(settings.duplex);
    dm.dmCollate = DEVMODE_COLLATE(if settings.collate { DMCOLLATE_TRUE } else { DMCOLLATE_FALSE });

    let hdc = unsafe {
        CreateDCW(
            PCWSTR::null(),
            PCWSTR::from_raw(printer_wide.as_ptr()),
            PCWSTR::null(),
            Some(&dm),
        )
    };

    if hdc.is_invalid() {
        return Err("create dc failed".into());
    }
    Ok(hdc)
}

// ═══════════════════════════════════════════════════════

pub fn list_printers() -> Vec<String> {
    unsafe {
        let mut needed = 0u32;
        let mut returned = 0u32;
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;

        let _ = EnumPrintersW(flags, PCWSTR::null(), 1, None, &mut needed, &mut returned);
        if needed == 0 {
            return vec![];
        }

        let mut buf: Vec<u8> = vec![0; needed as usize];
        let ok = EnumPrintersW(flags, PCWSTR::null(), 1, Some(&mut buf), &mut needed, &mut returned).is_ok();
        if !ok || returned == 0 {
            return vec![];
        }

        let info_ptr = buf.as_ptr() as *const PRINTER_INFO_1W;
        let mut printers = Vec::with_capacity(returned as usize);
        for i in 0..returned as usize {
            let info = info_ptr.add(i);
            if let Ok(s) = (*info).pName.to_string() {
                printers.push(s);
            }
        }
        printers
    }
}

// ═══════════════════════════════════════════════════════

pub fn default_printer() -> Option<String> {
    unsafe {
        let mut needed = 0u32;
        let _ = GetDefaultPrinterW(None, &mut needed);
        if needed == 0 {
            return None;
        }
        let mut buf: Vec<u16> = vec![0; needed as usize];
        let pwstr = PWSTR::from_raw(buf.as_mut_ptr());
        if GetDefaultPrinterW(Some(pwstr), &mut needed).as_bool() {
            Some(String::from_utf16_lossy(&buf[..needed as usize - 1]))
        } else {
            None
        }
    }
}

// ═══════════════════════════════════════════════════════

pub fn print_image(
    image_path: &str,
    printer_name: &str,
    settings: &PrintSettings,
) -> std::result::Result<(), String> {
    let img = image::open(image_path).map_err(|e| format!("open image: {e}"))?;
    let (iw, ih) = img.dimensions();
    let rgba = img.to_rgba8();

    let hdc = create_printer_dc(printer_name, settings)?;

    let pw = unsafe { GetDeviceCaps(Some(hdc), HORZRES) } as u32;
    let ph = unsafe { GetDeviceCaps(Some(hdc), VERTRES) } as u32;
    let ratio = f64::min(pw as f64 / iw as f64, ph as f64 / ih as f64);
    let tw = (iw as f64 * ratio) as i32;
    let th = (ih as f64 * ratio) as i32;

    let mut bgra = vec![0u8; (iw * ih * 4) as usize];
    let grayscale = settings.color_mode == DMCOLOR_MONOCHROME;
    for y in 0..ih {
        for x in 0..iw {
            let pixel = rgba.get_pixel(x, y);
            let idx = ((y * iw + x) * 4) as usize;
            if grayscale {
                let luma = (0.299f32 * pixel[0] as f32 + 0.587f32 * pixel[1] as f32 + 0.114f32 * pixel[2] as f32) as u8;
                bgra[idx] = luma;
                bgra[idx + 1] = luma;
                bgra[idx + 2] = luma;
            } else {
                bgra[idx] = pixel[2];
                bgra[idx + 1] = pixel[1];
                bgra[idx + 2] = pixel[0];
            }
            bgra[idx + 3] = pixel[3];
        }
    }

    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: iw as i32,
            biHeight: -(ih as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [RGBQUAD::default(); 1],
    };

    let doc_name: Vec<u16> = "PDF Print\0".encode_utf16().collect();
    let doc_info = DOCINFOW {
        cbSize: std::mem::size_of::<DOCINFOW>() as i32,
        lpszDocName: PCWSTR::from_raw(doc_name.as_ptr()),
        lpszOutput: PCWSTR::null(),
        lpszDatatype: PCWSTR::null(),
        fwType: 0,
    };

    unsafe {
        if StartDocW(hdc, &doc_info) <= 0 {
            let _ = DeleteDC(hdc);
            return Err("StartDoc 失败".into());
        }
        if StartPage(hdc) <= 0 {
            let _ = AbortDoc(hdc);
            let _ = DeleteDC(hdc);
            return Err("StartPage 失败".into());
        }
        StretchDIBits(
            hdc,
            0, 0, tw, th,
            0, 0, iw as i32, ih as i32,
            Some(bgra.as_ptr() as *const _),
            &bmi,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        if EndPage(hdc) <= 0 {
            let _ = AbortDoc(hdc);
            let _ = DeleteDC(hdc);
            return Err("EndPage 失败".into());
        }
        if EndDoc(hdc) <= 0 {
            let _ = DeleteDC(hdc);
            return Err("EndDoc 失败".into());
        }
        let _ = DeleteDC(hdc);
    }

    Ok(())
}
