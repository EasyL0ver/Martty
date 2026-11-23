//! Windows clipboard access (golem fork). Console terminals on Windows never
//! deliver a bracketed paste or an image to the app, so Ctrl+V reads the
//! clipboard here directly: a picture becomes a staged image, text becomes
//! a paste with its line breaks intact. Copy also writes the clipboard
//! directly instead of relying on the terminal's OSC 52 support.

use windows_sys::Win32::Foundation::{GlobalFree, HANDLE};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};

const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;

/// Holds the clipboard open; closes it on drop. Another process may hold
/// it for a moment, so opening retries briefly.
struct Open;

impl Open {
    fn new() -> Option<Open> {
        for _ in 0..10 {
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                return Some(Open);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        None
    }

    /// The bytes of one clipboard format, if present.
    fn bytes(&self, format: u32) -> Option<Vec<u8>> {
        if unsafe { IsClipboardFormatAvailable(format) } == 0 {
            return None;
        }
        let handle = unsafe { GetClipboardData(format) };
        if handle.is_null() {
            return None;
        }
        let size = unsafe { GlobalSize(handle) };
        let ptr = unsafe { GlobalLock(handle) } as *const u8;
        if ptr.is_null() {
            return None;
        }
        let data = unsafe { std::slice::from_raw_parts(ptr, size) }.to_vec();
        unsafe { GlobalUnlock(handle) };
        Some(data)
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        unsafe { CloseClipboard() };
    }
}

fn png_format() -> u32 {
    let name: Vec<u16> = "PNG".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { RegisterClipboardFormatW(name.as_ptr()) }
}

/// Clipboard text (CF_UNICODETEXT), if any.
pub fn read_text() -> Option<String> {
    let clip = Open::new()?;
    let bytes = clip.bytes(CF_UNICODETEXT)?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    Some(String::from_utf16_lossy(&units))
}

/// The clipboard picture as PNG bytes: the registered "PNG" format when an
/// app put one there (browsers, Snipping Tool), else the CF_DIB bitmap
/// every picture has, converted to PNG.
pub fn read_image_png() -> Option<Vec<u8>> {
    let (png, dib) = {
        let clip = Open::new()?;
        let png = clip.bytes(png_format()).filter(|b| b.starts_with(b"\x89PNG\r\n\x1a\n"));
        let dib = if png.is_none() { clip.bytes(CF_DIB) } else { None };
        (png, dib)
    };
    if png.is_some() {
        return png;
    }
    dib_to_png(&dib?)
}

/// Prefix a packed DIB (BITMAPINFOHEADER + masks + palette + pixels) with a
/// BMP file header, decode it and encode PNG.
pub fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>> {
    let u32_at = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(dib.get(at..at + 4)?.try_into().ok()?))
    };
    let header = u32_at(0)? as usize;
    let bits = u16::from_le_bytes(dib.get(14..16)?.try_into().ok()?);
    let compression = u32_at(16)?;
    let used = u32_at(32)? as usize;
    // BI_BITFIELDS / BI_ALPHABITFIELDS masks follow a 40-byte header.
    let masks = match (header, compression) {
        (40, 3) => 12,
        (40, 6) => 16,
        _ => 0,
    };
    let colors = if used > 0 {
        used
    } else if bits <= 8 {
        1usize << bits
    } else {
        0
    };
    let offset = 14 + header + masks + colors * 4;
    let total = 14 + dib.len();
    let mut bmp = Vec::with_capacity(total);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(total as u32).to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&(offset as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    let image = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp).ok()?;
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).ok()?;
    Some(png.into_inner())
}

/// Put `text` on the clipboard as CF_UNICODETEXT.
pub fn write_text(text: &str) -> bool {
    let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let Some(_clip) = Open::new() else {
        return false;
    };
    unsafe {
        if EmptyClipboard() == 0 {
            return false;
        }
        let mem = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
        if mem.is_null() {
            return false;
        }
        let ptr = GlobalLock(mem) as *mut u16;
        if ptr.is_null() {
            GlobalFree(mem);
            return false;
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
        GlobalUnlock(mem);
        if SetClipboardData(CF_UNICODETEXT, mem as HANDLE).is_null() {
            GlobalFree(mem);
            return false;
        }
    }
    true
}

#[cfg(test)]
#[path = "../tests/unit/winclip__tests.rs"]
mod tests;
