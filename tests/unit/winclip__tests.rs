use super::*;

/// A bottom-up 2x2 DIB with a 40-byte header; `pixels` rows bottom first.
fn dib(bits: u16, compression: u32, extra: &[u8], pixels: &[u8]) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(&40u32.to_le_bytes());
    d.extend_from_slice(&2i32.to_le_bytes());
    d.extend_from_slice(&2i32.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&bits.to_le_bytes());
    d.extend_from_slice(&compression.to_le_bytes());
    d.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    d.extend_from_slice(&[0; 16]);
    d.extend_from_slice(extra);
    d.extend_from_slice(pixels);
    d
}

fn decode(png: &[u8]) -> image::RgbaImage {
    image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .unwrap()
        .to_rgba8()
}

#[test]
fn dib_32bpp_rgb_converts_to_opaque_png() {
    // BGRX; the reserved byte is 0 and must not make the picture transparent.
    let pixels = [
        0, 0, 255, 0, 0, 255, 0, 0, // bottom row: red, green
        255, 0, 0, 0, 255, 255, 255, 0, // top row: blue, white
    ];
    let png = dib_to_png(&dib(32, 0, &[], &pixels)).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    let img = decode(&png);
    assert_eq!(img.dimensions(), (2, 2));
    assert_eq!(img.get_pixel(0, 1).0, [255, 0, 0, 255]);
    assert_eq!(img.get_pixel(1, 1).0, [0, 255, 0, 255]);
    assert_eq!(img.get_pixel(0, 0).0, [0, 0, 255, 255]);
    assert_eq!(img.get_pixel(1, 0).0, [255, 255, 255, 255]);
}

#[test]
fn dib_24bpp_with_row_padding_converts() {
    // 2 px * 3 bytes = 6, padded to 8 per row.
    let pixels = [0, 0, 255, 0, 255, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0];
    let img = decode(&dib_to_png(&dib(24, 0, &[], &pixels)).unwrap());
    assert_eq!(img.get_pixel(0, 1).0, [255, 0, 0, 255]);
    assert_eq!(img.get_pixel(1, 1).0, [0, 255, 0, 255]);
    assert_eq!(img.get_pixel(0, 0).0, [0, 0, 255, 255]);
    assert_eq!(img.get_pixel(1, 0).0, [0, 0, 0, 255]);
}

#[test]
fn dib_bitfields_masks_are_skipped() {
    let mut masks = Vec::new();
    for m in [0x00ff_0000u32, 0x0000_ff00, 0x0000_00ff] {
        masks.extend_from_slice(&m.to_le_bytes());
    }
    let pixels = [0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0];
    let img = decode(&dib_to_png(&dib(32, 3, &masks, &pixels)).unwrap());
    assert_eq!(img.get_pixel(0, 1).0, [255, 0, 0, 255]);
    assert_eq!(img.get_pixel(1, 0).0, [255, 255, 255, 255]);
}

#[test]
fn truncated_dib_is_rejected() {
    assert!(dib_to_png(&[40, 0, 0]).is_none());
}
