use std::path::Path;
use std::sync::Arc;

use image::{DynamicImage, RgbaImage};

use super::Payload;

pub fn render(path: &Path, box_size: (u32, u32)) -> Result<Payload, String> {
    let img = crate::fs::imgcodec::decode(path)?;
    let (cw, ch) = (img.width(), img.height());
    let (width, height, rgba) = finish(fit(img, box_size).to_rgba8());
    Ok(Payload::Image { width, height, source: (cw, ch), own: 1.0, vector: false, rgba, caption: format!("{cw} × {ch}") })
}

/// Shrink into `box_size`. Never enlarge: upscaling is the renderer's job and
/// keeps memory low.
pub fn fit(img: DynamicImage, box_size: (u32, u32)) -> DynamicImage {
    let (bw, bh) = (box_size.0.max(32), box_size.1.max(32));
    if img.width() > bw || img.height() > bh {
        img.resize(bw, bh, image::imageops::FilterType::Triangle)
    } else {
        img
    }
}

/// Opaque RGBA for the UI. Translucent pixels are laid over a checkerboard,
/// so black-on-clear icons don't vanish into a dark theme.
pub fn finish(mut img: RgbaImage) -> (u32, u32, Arc<Vec<u8>>) {
    const CELL: u32 = 16;
    if img.pixels().any(|p| p[3] < 255) {
        for (x, y, p) in img.enumerate_pixels_mut() {
            let a = u32::from(p[3]);
            if a == 255 {
                continue;
            }
            let bg = if (x / CELL + y / CELL).is_multiple_of(2) { 0xcc } else { 0x99 };
            for c in &mut p.0[..3] {
                *c = ((u32::from(*c) * a + bg * (255 - a) + 127) / 255) as u8;
            }
            p[3] = 255;
        }
    }
    (img.width(), img.height(), Arc::new(img.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lays_clear_pixels_over_a_checkerboard() {
        let mut img = RgbaImage::new(32, 32); // fully transparent
        img.put_pixel(0, 0, image::Rgba([10, 20, 30, 255]));
        let (_, _, rgba) = finish(img);
        assert_eq!(&rgba[..4], &[10, 20, 30, 255]); // opaque untouched
        assert_eq!(&rgba[4..8], &[0xcc, 0xcc, 0xcc, 255]); // first cell
        let second = (16 * 4) as usize;
        assert_eq!(&rgba[second..second + 4], &[0x99, 0x99, 0x99, 255]);
    }

    #[test]
    fn leaves_opaque_images_alone() {
        let img = RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]));
        let (_, _, rgba) = finish(img);
        assert!(rgba.chunks(4).all(|p| p == [1, 2, 3, 255]));
    }
}
