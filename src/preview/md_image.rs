//! Local pictures in rendered Markdown (#490): which `![alt](dest)` may be read, how big they are
//! drawn, and decoding them in the layout worker.
//!
//! Only files on this machine are read. A URL with a scheme other than `file:` is never fetched,
//! and nothing here opens a connection.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use image::imageops::FilterType;

use super::{DocImage, ImageKey};

/// Pictures tried per document, failures included.
pub(super) const MAX_ATTEMPTS: u32 = 32;
/// A file bigger than this is not read.
pub(super) const MAX_FILE_BYTES: u64 = 64 << 20;
/// A raster with more pixels than this is not decoded (its header says).
pub(super) const MAX_PIXELS: u64 = 40_000_000;
/// RGBA kept for all the pictures of one document. Once it is spent the rest stay labels.
pub(super) const MAX_RGBA_BYTES: usize = 24 << 20;
/// A picture is at most this many rows tall, or 60% of the pane's rows if that is less.
const MAX_ROWS: u32 = 24;
const MIN_ROWS: u32 = 3;

/// What the layout of pictures is worked out from, and what it has used up.
pub(super) struct Embed {
    /// The folder of the Markdown file, which a relative path is read from.
    pub dir: PathBuf,
    pub key: ImageKey,
    attempts: u32,
    rgba_bytes: usize,
}

impl Embed {
    pub fn new(dir: PathBuf, key: ImageKey) -> Self {
        Embed { dir, key, attempts: 0, rgba_bytes: 0 }
    }

    fn row_pt(&self) -> f32 {
        f32::from(self.key.row16) / 16.0
    }

    fn ppp(&self) -> f32 {
        (f32::from(self.key.ppp100) / 100.0).max(0.5)
    }

    /// The most rows a picture takes, in a pane of `key.rows`.
    fn cap_rows(&self) -> u32 {
        (u32::from(self.key.rows) * 6 / 10).clamp(MIN_ROWS, MAX_ROWS)
    }

    /// Read the picture `dest` names, shrunk to fit `avail_cols` cells across and the row cap
    /// down. `None` leaves the label: not a local file, missing, undecodable, or over a cap.
    pub fn load(&mut self, dest: &str, avail_cols: usize) -> Option<DocImage> {
        let path = local_path(dest, &self.dir)?;
        let meta = std::fs::metadata(&path).ok().filter(|m| m.is_file())?;
        if meta.len() > MAX_FILE_BYTES || self.attempts >= MAX_ATTEMPTS || self.rgba_bytes >= MAX_RGBA_BYTES {
            return None;
        }
        self.attempts += 1;
        let ppp = self.ppp();
        let box_pt = (avail_cols as f32 * f32::from(self.key.cell16) / 16.0, self.cap_rows() as f32 * self.row_pt());
        let svg = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        let (rgba, size_pt, source) = if svg {
            let data = std::fs::read(&path).ok()?;
            let (img, size_pt) = super::svg_preview::render_embedded(&data, box_pt, ppp).ok()?;
            (img, size_pt, None)
        } else {
            let (hw, hh) = crate::fs::imgcodec::dimensions(&path)?;
            if u64::from(hw) * u64::from(hh) > MAX_PIXELS {
                return None;
            }
            let img = crate::fs::imgcodec::decode(&path).ok()?;
            // The size after the EXIF turn `decode` applied: a portrait photo stored sideways is
            // wide in its header and tall here, and fitting it by the header would squash it.
            let (w, h) = (img.width(), img.height());
            let s = shrink((w as f32 / ppp, h as f32 / ppp), box_pt);
            let img = if s < 1.0 {
                let (tw, th) = (((w as f32 * s).round() as u32).max(1), ((h as f32 * s).round() as u32).max(1));
                img.resize_exact(tw, th, FilterType::Triangle)
            } else {
                img
            };
            let rgba = img.to_rgba8();
            let size_pt = (rgba.width() as f32 / ppp, rgba.height() as f32 / ppp);
            (rgba, size_pt, Some((w, h)))
        };
        let bytes = rgba.as_raw().len();
        if self.rgba_bytes + bytes > MAX_RGBA_BYTES {
            return None;
        }
        self.rgba_bytes += bytes;
        let rows = ((size_pt.1 / self.row_pt()).ceil() as u32).clamp(1, MAX_ROWS) as u16;
        let (width, height) = (rgba.width(), rgba.height());
        Some(DocImage {
            width,
            height,
            rgba: Arc::new(rgba.into_raw()),
            source: source.unwrap_or((width, height)),
            size_pt,
            rows,
            texture: Arc::new(OnceLock::new()),
        })
    }
}

/// The scale, at most 1, that fits a picture of `natural` points into `fit` points. It only
/// shrinks: a small icon stays small.
pub(super) fn shrink(natural: (f32, f32), fit: (f32, f32)) -> f32 {
    if natural.0 <= 0.0 || natural.1 <= 0.0 {
        return 1.0;
    }
    (fit.0 / natural.0).min(fit.1 / natural.1).clamp(f32::MIN_POSITIVE, 1.0)
}

/// The file a picture's destination names, if it names one on this machine: a relative path
/// (from `dir`), an absolute path, or a `file:` URL with no host. Anything else (`http:`,
/// `data:`, a network share, a device path) is `None`.
pub(super) fn local_path(dest: &str, dir: &Path) -> Option<PathBuf> {
    let dest = dest.trim();
    if dest.is_empty() {
        return None;
    }
    let b = dest.as_bytes();
    // `C:\x` and `C:/x` first: a drive letter is not a URL scheme.
    let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/');
    let rest = if drive {
        dest
    } else if let Some(scheme_end) = scheme_len(dest) {
        if !dest[..scheme_end].eq_ignore_ascii_case("file") {
            return None;
        }
        let after = dest[scheme_end + 1..].strip_prefix("//")?;
        let (host, path) = after.split_at(after.find('/').unwrap_or(after.len()));
        if !(host.is_empty() || host.eq_ignore_ascii_case("localhost")) {
            return None;
        }
        // `file:///C:/x` is `/C:/x`.
        let p = path.as_bytes();
        if p.len() >= 3 && p[0] == b'/' && p[1].is_ascii_alphabetic() && p[2] == b':' {
            &path[1..]
        } else {
            path
        }
    } else {
        dest
    };
    // A fragment or a query is not part of the file name.
    let rest = &rest[..rest.find(['#', '?']).unwrap_or(rest.len())];
    let decoded = percent_decode(rest)?;
    if decoded.is_empty() || decoded.contains('\0') {
        return None;
    }
    // A share (`\\host\x`, `//host/x`) or a device (`\\?\`, `\\.\`) is not "a file beside this one".
    let slashes = |s: &str| s.starts_with(['\\', '/']) && s[1..].starts_with(['\\', '/']);
    if slashes(&decoded) {
        return None;
    }
    let p = Path::new(&decoded);
    // `CON`, `COM1` or `nul.png` is a device in any folder on Windows: opening one waits for the
    // console or opens a port, and the preview worker would wait with it.
    #[cfg(windows)]
    if p.components().any(|c| matches!(c, std::path::Component::Normal(n) if is_device(n))) {
        return None;
    }
    Some(if p.is_absolute() { p.to_path_buf() } else { dir.join(p) })
}

/// Whether a file name is a Windows device name, with or without an extension.
#[cfg(windows)]
fn is_device(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else { return false };
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
    // ito keeps the list (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`) and puts a `_`
    // in front of such a name; the console's own two names are not in it.
    (stem.len() <= 4 && ito_common::safe_file_name(stem) == format!("_{stem}"))
        || stem.eq_ignore_ascii_case("CONIN$")
        || stem.eq_ignore_ascii_case("CONOUT$")
}

/// Length of a URL scheme at the start of `s` (`http` in `http://x`), if it has one.
fn scheme_len(s: &str) -> Option<usize> {
    let end = s.find(':')?;
    let scheme = &s[..end];
    let mut chars = scheme.chars();
    let ok =
        chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    ok.then_some(end)
}

/// `%20` and the like decoded; a malformed escape is left as written. `None` if the bytes are not UTF-8.
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        match (b[i], b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (c, ..) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        PathBuf::from(if cfg!(windows) { r"C:\docs" } else { "/docs" })
    }

    #[test]
    fn a_relative_path_is_read_from_the_markdown_folder() {
        assert_eq!(local_path("a/b.png", &dir()), Some(dir().join("a/b.png")));
        assert_eq!(local_path("my%20pic.png#frag", &dir()), Some(dir().join("my pic.png")));
        assert_eq!(local_path("x.png?size=2", &dir()), Some(dir().join("x.png")));
    }

    #[test]
    fn remote_and_odd_schemes_are_never_local() {
        for d in ["http://a/b.png", "https://a/b.png", "HTTPS://a/b.png", "data:image/png;base64,AAAA", "ftp://a/b.png", "javascript:x"] {
            assert_eq!(local_path(d, &dir()), None, "{d}");
        }
    }

    #[test]
    fn a_drive_letter_is_not_a_scheme() {
        assert!(local_path("C:/pics/a.png", &dir()).is_some());
        assert!(local_path(r"C:\pics\a.png", &dir()).is_some());
    }

    #[test]
    fn file_urls_need_no_host() {
        assert!(local_path("file:///tmp/a%20b.png", &dir()).is_some());
        assert!(local_path("file://localhost/tmp/a.png", &dir()).is_some());
        assert_eq!(local_path("file://server/share/a.png", &dir()), None);
        assert_eq!(local_path("file:////server/share/a.png", &dir()), None);
        assert_eq!(local_path("file:///C:/pics/a.png", &dir()).unwrap().file_name().unwrap(), "a.png");
    }

    #[test]
    fn shares_and_devices_are_refused() {
        for d in [r"\\host\share\a.png", "//host/share/a.png", r"\\?\C:\a.png", r"\\.\pipe\x", "%5C%5Chost%5Cx.png"] {
            assert_eq!(local_path(d, &dir()), None, "{d}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn device_names_are_refused() {
        for d in ["CON", "nul.png", "sub/COM1.jpg", "aux .png", "CONIN$", r"C:\x\lpt9.svg"] {
            assert_eq!(local_path(d, &dir()), None, "{d}");
        }
        for d in ["con1.png", "console.png", "COM0.png", "_CON.png", "prn-shot.png"] {
            assert!(local_path(d, &dir()).is_some(), "{d}");
        }
    }

    /// A photo stored 200×100 with EXIF orientation 6 is 100×200 upright. Fitting it into a box
    /// 80pt wide must keep that shape (80×160), not squash the upright picture into the stored one's.
    #[test]
    fn a_turned_photo_keeps_its_upright_shape() {
        let dir = crate::util::test_dir("md-image-orient");
        let mut jpeg = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(200, 100))
            .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .unwrap();
        // Little-endian TIFF IFD with Orientation = 6 (rotate 90 degrees).
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut turned = jpeg[..2].to_vec();
        turned.extend_from_slice(&[0xff, 0xe1]);
        turned.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        turned.extend_from_slice(exif);
        turned.extend_from_slice(&jpeg[2..]);
        std::fs::write(dir.join("turned.jpg"), turned).unwrap();
        // 16pt rows, 8pt cells, a 20-row pane (12 rows at most, 192pt), 1 pixel per point.
        let key = ImageKey { row16: 16 * 16, cell16: 8 * 16, rows: 20, ppp100: 100 };
        let img = Embed::new(dir.clone(), key).load("turned.jpg", 10).expect("drawn");
        assert_eq!((img.width, img.height), (80, 160));
        assert_eq!(img.source, (100, 200), "the upright size");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shrink_never_enlarges() {
        assert_eq!(shrink((10.0, 10.0), (100.0, 100.0)), 1.0);
        assert_eq!(shrink((200.0, 100.0), (100.0, 100.0)), 0.5);
        assert_eq!(shrink((100.0, 400.0), (200.0, 100.0)), 0.25);
    }
}
