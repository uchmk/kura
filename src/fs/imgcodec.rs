//! Shared, bounded image decoding and pure Rust image encoders.
use std::io::Write;
use std::path::Path;

use image::{DynamicImage, ImageDecoder, ImageEncoder};

pub const MAX_PIXELS: u64 = 80_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    WebP,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::WebP => "webp",
        }
    }
    pub fn matches(self, format: image::ImageFormat) -> bool {
        matches!(
            (self, format),
            (Self::Jpeg, image::ImageFormat::Jpeg) | (Self::Png, image::ImageFormat::Png) | (Self::WebP, image::ImageFormat::WebP)
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PngLevel {
    Fast,
    #[default]
    Default,
    Best,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodeOpts {
    pub jpeg_quality: u8,
    pub png_level: PngLevel,
    pub alpha_bg: [u8; 3],
    pub delete_original: bool,
}

impl Default for EncodeOpts {
    fn default() -> Self {
        Self { jpeg_quality: 85, png_level: PngLevel::Default, alpha_bg: [255; 3], delete_original: false }
    }
}

pub fn decode(path: &Path) -> Result<DynamicImage, String> {
    let mut decoder = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_decoder()
        .map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(format!("image too large ({w}x{h})"));
    }
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// Sniff the contents rather than trusting a file extension.
/// `Ok(None)` means readable but not an image; `Err` means it could not be read
/// (permissions, lock, I/O), which the caller reports instead of skipping silently.
pub fn source_format(path: &Path) -> Result<Option<image::ImageFormat>, String> {
    Ok(image::ImageReader::open(path).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?.format())
}

pub fn encode(img: &DynamicImage, fmt: ImageFormat, opts: &EncodeOpts, out: &mut impl Write) -> Result<(), String> {
    let result = match fmt {
        ImageFormat::Jpeg => {
            let rgba = img.to_rgba8();
            let rgb = image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
                let p = rgba.get_pixel(x, y);
                let a = u32::from(p[3]);
                image::Rgb(std::array::from_fn(|i| ((u32::from(p[i]) * a + u32::from(opts.alpha_bg[i]) * (255 - a) + 127) / 255) as u8))
            });
            image::codecs::jpeg::JpegEncoder::new_with_quality(out, opts.jpeg_quality.clamp(1, 100)).encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
        }
        ImageFormat::Png => {
            let compression = match opts.png_level {
                PngLevel::Fast => image::codecs::png::CompressionType::Fast,
                PngLevel::Default => image::codecs::png::CompressionType::Default,
                PngLevel::Best => image::codecs::png::CompressionType::Best,
            };
            // Retain 16-bit channels in PNG when the input has them.
            let png = if img.color().bits_per_pixel() / img.color().channel_count() as u16 > 8 {
                DynamicImage::ImageRgba16(img.to_rgba16())
            } else {
                DynamicImage::ImageRgba8(img.to_rgba8())
            };
            image::codecs::png::PngEncoder::new_with_quality(out, compression, image::codecs::png::FilterType::Adaptive).write_image(
                png.as_bytes(),
                png.width(),
                png.height(),
                png.color().into(),
            )
        }
        ImageFormat::WebP => {
            let rgba = img.to_rgba8();
            image::codecs::webp::WebPEncoder::new_lossless(out).write_image(
                rgba.as_raw(),
                rgba.width(),
                rgba.height(),
                image::ExtendedColorType::Rgba8,
            )
        }
    };
    result.map_err(|e| e.to_string())
}

/// The guard owns only the staging file; the old destination survives all failures.
struct Staging(std::path::PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub fn write_image(
    img: &DynamicImage,
    fmt: ImageFormat,
    opts: &EncodeOpts,
    dest: &Path,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Option<u64>, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let (stage, mut file) = loop {
        let path = dest.with_file_name(format!(".kura-convert-{}-{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(f) => break (Staging(path), f),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    if cancelled() {
        return Ok(None);
    }
    struct CheckedWriter<'a, W, F> {
        out: &'a mut W,
        cancelled: &'a mut F,
    }
    impl<W: Write, F: FnMut() -> bool> Write for CheckedWriter<'_, W, F> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if (self.cancelled)() {
                return Err(std::io::Error::other("conversion cancelled"));
            }
            self.out.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.out.flush()
        }
    }
    if let Err(e) = encode(img, fmt, opts, &mut CheckedWriter { out: &mut file, cancelled: &mut cancelled }) {
        if cancelled() {
            return Ok(None);
        }
        return Err(e);
    }
    file.flush().map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    drop(file);
    if cancelled() {
        return Ok(None);
    }
    std::fs::rename(&stage.0, dest).map_err(|e| e.to_string())?;
    Ok(Some(size))
}

pub fn percent_change(before: u64, after: u64) -> String {
    if before == 0 {
        return "0%".into();
    }
    let delta = (after as f64 / before as f64 - 1.0) * 100.0;
    format!("{delta:+.0}%")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> DynamicImage {
        DynamicImage::ImageRgba8(image::RgbaImage::from_fn(64, 48, |x, y| {
            image::Rgba([(x * 17 + y * 7) as u8, (x * 3 + y * 19) as u8, (x * 11 + y * 13) as u8, if x < 32 { 0 } else { 255 }])
        }))
    }

    #[test]
    fn all_encoders_round_trip_dimensions_and_lossless_pixels() {
        let dir = crate::util::test_dir("convert-codecs");
        let source = dir.join("source.png");
        sample().save(&source).unwrap();
        let img = decode(&source).unwrap();
        for fmt in [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP] {
            let dest = dir.join(format!("out.{}", fmt.extension()));
            assert!(write_image(&img, fmt, &EncodeOpts::default(), &dest, || false).unwrap().is_some());
            let back = decode(&dest).unwrap();
            assert_eq!((back.width(), back.height()), (64, 48));
            if fmt != ImageFormat::Jpeg {
                assert_eq!(back.to_rgba8(), img.to_rgba8());
            }
        }
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 4);
    }

    #[test]
    fn jpeg_quality_changes_size_and_alpha_is_composited() {
        let img = sample();
        let mut sizes = Vec::new();
        for jpeg_quality in [10, 50, 95] {
            let mut out = Vec::new();
            encode(&img, ImageFormat::Jpeg, &EncodeOpts { jpeg_quality, alpha_bg: [200, 100, 50], ..Default::default() }, &mut out)
                .unwrap();
            let back = image::load_from_memory(&out).unwrap().to_rgb8();
            let pixel = back.get_pixel(0, 0);
            for (actual, expected) in pixel.0.into_iter().zip([200u8, 100, 50]) {
                assert!(actual.abs_diff(expected) <= 10);
            }
            sizes.push(out.len());
        }
        assert!(sizes.windows(2).all(|s| s[0] < s[1]), "{sizes:?}");
        let half = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(32, 32, image::Rgba([0, 0, 0, 128])));
        let mut out = Vec::new();
        encode(&half, ImageFormat::Jpeg, &EncodeOpts::default(), &mut out).unwrap();
        let back = image::load_from_memory(&out).unwrap().to_rgb8();
        assert!(back.get_pixel(16, 16)[0].abs_diff(127) <= 2);
    }

    #[test]
    fn cancellation_and_rename_failure_preserve_output_and_clean_staging() {
        let dir = crate::util::test_dir("convert-atomic");
        let dest = dir.join("out.jpg");
        std::fs::write(&dest, b"original").unwrap();
        let mut calls = 0;
        let result = write_image(&sample(), ImageFormat::Jpeg, &EncodeOpts::default(), &dest, || {
            calls += 1;
            calls > 2
        })
        .unwrap();
        assert!(result.is_none());
        assert_eq!(std::fs::read(&dest).unwrap(), b"original");
        let folder = dir.join("folder");
        std::fs::create_dir(&folder).unwrap();
        assert!(write_image(&sample(), ImageFormat::Png, &EncodeOpts::default(), &folder, || false).is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        // Overwriting is a rename of a finished image, including on Windows.
        assert!(write_image(&sample(), ImageFormat::Jpeg, &EncodeOpts::default(), &dest, || false).unwrap().is_some());
        assert!(decode(&dest).is_ok());
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 2);
    }

    #[test]
    fn percentage_has_a_sign_and_handles_empty_inputs() {
        assert_eq!(percent_change(400, 100), "-75%");
        assert_eq!(percent_change(100, 150), "+50%");
        assert_eq!(percent_change(0, 0), "0%");
    }

    #[test]
    fn decoding_bakes_exif_orientation_and_rejects_excessive_dimensions() {
        let dir = crate::util::test_dir("convert-orientation");
        let mut jpeg = Vec::new();
        encode(&sample(), ImageFormat::Jpeg, &EncodeOpts::default(), &mut jpeg).unwrap();
        // Little-endian TIFF IFD with Orientation = 6 (rotate 90 degrees).
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut oriented = jpeg[..2].to_vec();
        oriented.extend_from_slice(&[0xff, 0xe1]);
        oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        oriented.extend_from_slice(exif);
        oriented.extend_from_slice(&jpeg[2..]);
        let path = dir.join("rotated.jpg");
        std::fs::write(&path, oriented).unwrap();
        let back = decode(&path).unwrap();
        assert_eq!((back.width(), back.height()), (48, 64));
        let sof = jpeg.windows(2).position(|b| b == [0xff, 0xc0]).unwrap();
        jpeg[sof + 5..sof + 7].copy_from_slice(&9000u16.to_be_bytes());
        jpeg[sof + 7..sof + 9].copy_from_slice(&9000u16.to_be_bytes());
        std::fs::write(&path, jpeg).unwrap();
        assert!(decode(&path).unwrap_err().contains("image too large"));
    }

    #[test]
    fn png_preserves_sixteen_bit_channels() {
        let img = DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(3, 2, image::Rgba([12345u16, 23456, 34567, 45678])));
        let mut out = Vec::new();
        encode(&img, ImageFormat::Png, &EncodeOpts::default(), &mut out).unwrap();
        assert_eq!(image::load_from_memory(&out).unwrap().to_rgba16(), img.to_rgba16());
    }
}
