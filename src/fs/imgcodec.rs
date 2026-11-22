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
    /// 0 = lossless WebP; 1-100 = lossy through `ito-image`.
    pub webp_quality: u8,
    pub png_level: PngLevel,
    pub alpha_bg: [u8; 3],
    pub delete_original: bool,
}

impl Default for EncodeOpts {
    fn default() -> Self {
        Self { jpeg_quality: 85, webp_quality: 0, png_level: PngLevel::Default, alpha_bg: [255; 3], delete_original: false }
    }
}

/// A JPEG XL file bigger than this is refused before it is read.
const MAX_JXL_BYTES: u64 = 256 << 20;

/// What a readable image file is. The `image` crate has no JPEG XL, so it has its own arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Image(image::ImageFormat),
    Jxl,
}

impl Source {
    /// Whether the file is already in the target format. JPEG XL never is: kura writes none.
    pub fn is(self, format: ImageFormat) -> bool {
        matches!(self, Self::Image(f) if format.matches(f))
    }
}

/// The first bytes of the file (up to `n`), enough to sniff a format.
fn head(path: &Path, n: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(path).map_err(|e| e.to_string())?.take(n as u64).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

fn decode_jxl_file(path: &Path) -> Result<DynamicImage, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > MAX_JXL_BYTES {
        return Err(format!("JPEG XL file too large ({} MiB)", len >> 20));
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    // Already upright: `decode_jxl` applies the orientation, so none is applied again here.
    let rgba = ito_image::decode_jxl(&bytes, MAX_PIXELS).map_err(|e| match e {
        ito_image::Error::TooLarge(w, h) => format!("image too large ({w}x{h})"),
        e => e.to_string(),
    })?;
    image::RgbaImage::from_raw(rgba.width, rgba.height, rgba.pixels).map(DynamicImage::ImageRgba8).ok_or_else(|| "invalid JPEG XL".into())
}

/// Decodes by content, not by extension. Worker threads only: JPEG XL has no time limit.
pub fn decode(path: &Path) -> Result<DynamicImage, String> {
    if ito_image::is_jxl(&head(path, 12)?) {
        return decode_jxl_file(path);
    }
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
pub fn source_format(path: &Path) -> Result<Option<Source>, String> {
    if ito_image::is_jxl(&head(path, 12)?) {
        return Ok(Some(Source::Jxl));
    }
    Ok(image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .format()
        .map(Source::Image))
}

/// The size of a JPEG XL file as its header gives it, or `None`.
///
/// `max_pixels` is 0 on purpose: `decode_jxl` reads the header first and returns
/// `TooLarge(w, h)` before decoding a pixel when the picture is over the limit, so any real
/// picture gives its size without the cost of decoding it. Only the first 64 KiB is read, and
/// only after the first 12 bytes say JPEG XL: Details and spot ask this of every picture.
///
/// The header has to be inside what is read. A container can put big Exif, XMP or compressed
/// (`brob`) boxes before the codestream, and an embedded ICC profile is part of the header, so a
/// miss in the first 64 KiB is tried once more with up to 4 MiB. Never the whole file.
pub fn jxl_size(path: &Path) -> Option<(u32, u32)> {
    if !ito_image::is_jxl(&head(path, 12).ok()?) {
        return None;
    }
    let len = std::fs::metadata(path).ok()?.len();
    for n in [64usize << 10, 4 << 20] {
        let bytes = head(path, n).ok()?;
        if let Err(ito_image::Error::TooLarge(w, h)) = ito_image::decode_jxl(&bytes, 0) {
            return Some((w, h));
        }
        if len <= n as u64 {
            break;
        }
    }
    None
}

/// A picture's pixel size from its header, for every format `decode` reads.
pub fn dimensions(path: &Path) -> Option<(u32, u32)> {
    if let Some(size) = jxl_size(path) {
        return Some(size);
    }
    image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.into_dimensions().ok()
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
        ImageFormat::WebP if opts.webp_quality > 0 => {
            let rgba = img.to_rgba8();
            let (width, height) = (rgba.width(), rgba.height());
            let picture = ito_image::Rgba::new(width, height, rgba.into_raw()).map_err(|e| e.to_string())?;
            let bytes = ito_image::encode_webp(&picture, f32::from(opts.webp_quality.min(100))).map_err(|e| e.to_string())?;
            return out.write_all(&bytes).map_err(|e| e.to_string());
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
    fn webp_quality_zero_is_lossless_and_positive_is_lossy_with_alpha() {
        let img = sample();
        let mut lossless = Vec::new();
        encode(&img, ImageFormat::WebP, &EncodeOpts::default(), &mut lossless).unwrap();
        let back = image::load_from_memory(&lossless).unwrap();
        assert_eq!(back.to_rgba8(), img.to_rgba8());
        let mut lossy = Vec::new();
        encode(&img, ImageFormat::WebP, &EncodeOpts { webp_quality: 75, ..Default::default() }, &mut lossy).unwrap();
        assert_ne!(lossy, lossless);
        let back = image::load_from_memory(&lossy).unwrap();
        assert_eq!((back.width(), back.height()), (64, 48));
        let translucent = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(64, 48, |x, y| {
            image::Rgba([(x * 4) as u8, (y * 5) as u8, 90, if x < 32 { 128 } else { 255 }])
        }));
        let mut out = Vec::new();
        encode(&translucent, ImageFormat::WebP, &EncodeOpts { webp_quality: 75, ..Default::default() }, &mut out).unwrap();
        let back = image::load_from_memory(&out).unwrap().to_rgba8();
        let mean = |half: std::ops::Range<u32>| {
            let alphas: Vec<u32> = back.enumerate_pixels().filter(|(x, _, _)| half.contains(x)).map(|(_, _, p)| u32::from(p[3])).collect();
            alphas.iter().sum::<u32>() / alphas.len() as u32
        };
        assert!((112..=144).contains(&mean(0..32)), "translucent half: mean alpha {}", mean(0..32));
        assert!(mean(32..64) >= 240, "opaque half: mean alpha {}", mean(32..64));
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

    fn jxl_fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jxl").join(name)
    }

    #[test]
    fn jpeg_xl_fixtures_decode_to_their_size() {
        let size = |name: &str| {
            let img = decode(&jxl_fixture(name)).unwrap();
            (img.width(), img.height())
        };
        assert_eq!(size("lossless.jxl"), (16, 8));
        let alpha = decode(&jxl_fixture("alpha.jxl")).unwrap().to_rgba8();
        assert_eq!(alpha.dimensions(), (16, 8));
        assert!(alpha.pixels().any(|p| p[3] > 0 && p[3] < 255), "translucent pixels are kept");
        assert_eq!(size("anim.jxl"), (16, 8), "frame 0");
        // Stored 16x8 with orientation 6: upright it is 8x16, and the header size says so too.
        assert_eq!(size("orient6.jxl"), (8, 16));
    }

    #[test]
    fn jpeg_xl_size_from_the_header_matches_the_decoded_size() {
        for name in ["lossless.jxl", "alpha.jxl", "anim.jxl", "orient6.jxl"] {
            let img = decode(&jxl_fixture(name)).unwrap();
            let size = jxl_size(&jxl_fixture(name)).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(size, (img.width(), img.height()), "{name}");
            assert_eq!(dimensions(&jxl_fixture(name)), Some(size));
        }
    }

    #[test]
    fn jpeg_xl_size_is_found_behind_a_big_metadata_box() {
        // A container with 100 KiB of XMP before the codestream: the header is past the first 64 KiB.
        let boxed = |kind: &[u8; 4], body: &[u8]| {
            let mut b = ((8 + body.len()) as u32).to_be_bytes().to_vec();
            b.extend_from_slice(kind);
            b.extend_from_slice(body);
            b
        };
        let mut file = boxed(b"JXL ", &[0x0d, 0x0a, 0x87, 0x0a]);
        file.extend(boxed(b"ftyp", b"jxl \0\0\0\0jxl "));
        file.extend(boxed(b"xml ", &vec![b' '; 100 << 10]));
        file.extend(boxed(b"jxlc", &std::fs::read(jxl_fixture("lossless.jxl")).unwrap()));
        let dir = crate::util::test_dir("jxl-big-box");
        let path = dir.join("boxed.jxl");
        std::fs::write(&path, file).unwrap();
        assert_eq!(jxl_size(&path), Some((16, 8)));
        assert_eq!(dimensions(&path), Some((16, 8)));
        let img = decode(&path).unwrap();
        assert_eq!((img.width(), img.height()), (16, 8));
    }

    #[test]
    fn content_wins_over_the_extension() {
        let dir = crate::util::test_dir("jxl-content");
        std::fs::copy(jxl_fixture("lossless.jxl"), dir.join("x.bin")).unwrap();
        assert!(decode(&dir.join("x.bin")).is_ok());
        assert_eq!(source_format(&dir.join("x.bin")).unwrap(), Some(Source::Jxl));
        let png = dir.join("p.jxl");
        sample().save_with_format(&png, image::ImageFormat::Png).unwrap();
        let back = decode(&png).unwrap();
        assert_eq!((back.width(), back.height()), (64, 48));
        assert_eq!(source_format(&png).unwrap(), Some(Source::Image(image::ImageFormat::Png)));
    }

    #[test]
    fn broken_jpeg_xl_is_an_error_and_has_no_size() {
        let dir = crate::util::test_dir("jxl-garbage");
        let path = dir.join("bad.jxl");
        let mut bytes = vec![0xff, 0x0a];
        bytes.extend((0..200u32).map(|i| (i * 37 + 11) as u8));
        std::fs::write(&path, bytes).unwrap();
        assert!(decode(&path).is_err());
        assert_eq!(jxl_size(&path), None);
        assert_eq!(dimensions(&path), None);
    }

    #[test]
    fn jpeg_xl_converts_to_png_and_is_never_its_own_target() {
        let dir = crate::util::test_dir("jxl-convert");
        let src = jxl_fixture("lossless.jxl");
        let source = source_format(&src).unwrap().unwrap();
        assert_eq!(source, Source::Jxl);
        assert!(![ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP].into_iter().any(|f| source.is(f)));
        let img = decode(&src).unwrap();
        let dest = dir.join("out.png");
        assert!(write_image(&img, ImageFormat::Png, &EncodeOpts::default(), &dest, || false).unwrap().is_some());
        let back = decode(&dest).unwrap();
        assert_eq!((back.width(), back.height()), (img.width(), img.height()));
        // The PNG it wrote is its own target, so converting it to PNG again is skipped.
        let written = source_format(&dest).unwrap().unwrap();
        assert_eq!(written, Source::Image(image::ImageFormat::Png));
        assert!(written.is(ImageFormat::Png));
        assert!(!written.is(ImageFormat::Jpeg));
    }

    #[test]
    fn png_preserves_sixteen_bit_channels() {
        let img = DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(3, 2, image::Rgba([12345u16, 23456, 34567, 45678])));
        let mut out = Vec::new();
        encode(&img, ImageFormat::Png, &EncodeOpts::default(), &mut out).unwrap();
        assert_eq!(image::load_from_memory(&out).unwrap().to_rgba16(), img.to_rgba16());
    }
}
