//! Lazy metadata. All filesystem and account lookups run on the worker.
use super::Entry;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Details {
    pub owner: String,
    pub dimensions: Option<(u32, u32)>,
}

pub fn read(entry: &Entry) -> Details {
    Details { owner: owner(&entry.path), dimensions: dimensions(entry) }
}

/// A regular file whose extension kura reads picture dimensions for.
pub(crate) fn reads_dimensions(entry: &Entry) -> bool {
    entry.kind == super::Kind::File
        && entry.ext.as_deref().is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "bmp"
                    | "ico"
                    | "tif"
                    | "tiff"
                    | "webp"
                    | "pnm"
                    | "ppm"
                    | "pgm"
                    | "pbm"
                    | "pam"
                    | "qoi"
                    | "jxl"
            )
        })
}

fn dimensions(entry: &Entry) -> Option<(u32, u32)> {
    if !reads_dimensions(entry) {
        return None;
    }
    super::imgcodec::dimensions(&entry.path)
}

#[cfg(unix)]
fn owner(path: &Path) -> String {
    use std::os::unix::fs::MetadataExt;
    use std::sync::OnceLock;
    static USERS: OnceLock<std::collections::HashMap<u32, String>> = OnceLock::new();
    let Ok(md) = std::fs::symlink_metadata(path) else { return String::new() };
    let users = USERS.get_or_init(|| {
        std::fs::read_to_string("/etc/passwd")
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let mut fields = line.split(':');
                let name = fields.next()?;
                fields.next()?;
                Some((fields.next()?.parse().ok()?, name.to_owned()))
            })
            .collect()
    });
    users.get(&md.uid()).cloned().unwrap_or_else(|| md.uid().to_string())
}

#[cfg(windows)]
fn owner(path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;
    use std::sync::{Mutex, OnceLock};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows::Win32::Security::{GetLengthSid, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID};
    static CACHE: OnceLock<Mutex<std::collections::HashMap<Vec<u8>, String>>> = OnceLock::new();
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut sid = PSID::default();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        if GetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut sid),
            None,
            None,
            None,
            &mut descriptor,
        )
        .is_err()
        {
            return String::new();
        }
        if sid.0.is_null() {
            let _ = LocalFree(Some(HLOCAL(descriptor.0)));
            return String::new();
        }
        // The descriptor owns the SID until LocalFree; copy the SID for the cache key.
        let key = std::slice::from_raw_parts(sid.0.cast::<u8>(), GetLengthSid(sid) as usize).to_vec();
        let cache = CACHE.get_or_init(Default::default);
        let cached = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key).cloned();
        let result = cached.unwrap_or_else(|| {
            let result = sid_label(sid);
            cache.lock().unwrap_or_else(|e| e.into_inner()).insert(key, result.clone());
            result
        });
        let _ = LocalFree(Some(HLOCAL(descriptor.0)));
        result
    }
}

/// The account a SID names, as the Owner column shows it. A SID no account
/// answers to any more (a deleted user, a domain out of reach) reads as the SID
/// itself, `S-1-5-21-…`, rather than as nothing: the uid
/// number does the same on Unix.
#[cfg(windows)]
fn sid_label(sid: windows::Win32::Security::PSID) -> String {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows::Win32::Security::{LookupAccountSidW, SID_NAME_USE};
    unsafe {
        let (mut names, mut domains) = (0, 0);
        let mut use_ = SID_NAME_USE::default();
        let _ = LookupAccountSidW(PCWSTR::null(), sid, None, &mut names, None, &mut domains, &mut use_);
        let mut name = vec![0u16; names as usize];
        let mut domain = vec![0u16; domains as usize];
        if LookupAccountSidW(
            PCWSTR::null(),
            sid,
            Some(PWSTR(name.as_mut_ptr())),
            &mut names,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut domains,
            &mut use_,
        )
        .is_ok()
        {
            let name = String::from_utf16_lossy(&name[..names as usize]);
            let domain = String::from_utf16_lossy(&domain[..domains as usize]);
            return account_label(&domain, &name, &std::env::var("COMPUTERNAME").unwrap_or_default());
        }
        let mut text = PWSTR::null();
        if ConvertSidToStringSidW(sid, &mut text).is_err() || text.is_null() {
            return String::new();
        }
        let label = text.to_string().unwrap_or_default();
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        label
    }
}

/// `DOMAIN\name` as the owner is shown. The computer's own name and the
/// built-in authorities say nothing the reader does not know, and they take
/// most of a narrow column, so a local account is just `name`. A real domain
/// account keeps its domain.
#[cfg_attr(not(windows), allow(dead_code))]
fn account_label(domain: &str, name: &str, computer: &str) -> String {
    const LOCAL: [&str; 5] = ["BUILTIN", "NT AUTHORITY", "NT SERVICE", "Window Manager", "Font Driver Host"];
    let local = domain.is_empty()
        || (!computer.is_empty() && domain.eq_ignore_ascii_case(computer))
        || LOCAL.iter().any(|l| domain.eq_ignore_ascii_case(l));
    if local {
        name.to_owned()
    } else {
        format!("{domain}\\{name}")
    }
}

#[cfg(not(any(windows, unix)))]
fn owner(_path: &Path) -> String {
    String::new()
}

pub fn detail_type(entry: &Entry, lang: &str) -> String {
    let ext = entry.ext.as_deref().unwrap_or("").to_lowercase();
    let key = match entry.kind {
        super::Kind::Dir => "details.folder",
        super::Kind::Link { .. } => "details.link",
        super::Kind::File => match ext.as_str() {
            "" => "details.file",
            "md" | "markdown" => "details.markdown",
            "png" => "details.png",
            "jpg" | "jpeg" => "details.jpeg",
            _ => return crate::i18n::trf(lang, "details.ext_file", &[&ext.to_uppercase()]),
        },
    };
    crate::i18n::tr(lang, key)
}

pub fn permissions(entry: &Entry) -> String {
    let kind = match entry.kind {
        super::Kind::Dir => 'd',
        super::Kind::Link { .. } => 'l',
        super::Kind::File => '-',
    };
    format!("{kind}r{}{}", if entry.readonly { "-" } else { "w" }, if entry.hidden { "h" } else { "" })
}

/// One worker per application; queued requests and interrupted batches are newest-wins.
pub struct Worker {
    tx: crossbeam_channel::Sender<(u64, Vec<Entry>)>,
    pub rx: crossbeam_channel::Receiver<(u64, Entry, Details)>,
    generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
}
impl Worker {
    pub fn new(ctx: egui::Context) -> Self {
        use std::sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        };
        let (tx, requests) = crossbeam_channel::unbounded::<(u64, Vec<Entry>)>();
        let (results, rx) = crossbeam_channel::unbounded();
        let generation = Arc::new(AtomicU64::new(0));
        let current = generation.clone();
        std::thread::spawn(move || {
            while let Ok(mut request) = requests.recv() {
                while let Ok(newer) = requests.try_recv() {
                    request = newer;
                }
                let (id, entries) = request;
                for entry in entries {
                    if current.load(Ordering::Relaxed) != id {
                        break;
                    }
                    // Sent even when a newer request arrived meanwhile: the read is
                    // still right for its path, and the app drops it by generation.
                    let details = read(&entry);
                    if results.send((id, entry, details)).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            }
        });
        Self { tx, rx, generation }
    }
    pub fn request(&self, entries: Vec<Entry>) -> u64 {
        let id = self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let _ = self.tx.send((id, entries));
        id
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_headers_supply_dimensions() {
        let dir = crate::util::test_dir("details-image");
        let path = dir.join("small.png");
        image::RgbImage::new(3, 2).save(&path).unwrap();
        let entry = Entry::from_path(path.clone()).unwrap();
        assert_eq!(read(&entry).dimensions, Some((3, 2)));
        std::fs::write(&path, b"broken").unwrap();
        assert_eq!(read(&entry).dimensions, None);
    }
    #[test]
    fn a_local_account_loses_its_computer_name() {
        assert_eq!(account_label("DESKTOP-7UBRVG9", "yuu06", "desktop-7ubrvg9"), "yuu06");
        assert_eq!(account_label("NT AUTHORITY", "SYSTEM", "PC"), "SYSTEM");
        assert_eq!(account_label("BUILTIN", "Administrators", "PC"), "Administrators");
        assert_eq!(account_label("NT SERVICE", "TrustedInstaller", "PC"), "TrustedInstaller");
        assert_eq!(account_label("Window Manager", "DWM-1", "PC"), "DWM-1");
        assert_eq!(account_label("Font Driver Host", "UMFD-0", "PC"), "UMFD-0");
        assert_eq!(account_label("", "alone", "PC"), "alone");
        assert_eq!(account_label("CORP", "yuu06", "PC"), "CORP\\yuu06", "a domain account keeps its domain");
        assert_eq!(account_label("CORP", "yuu06", ""), "CORP\\yuu06");
    }
    #[cfg(any(windows, unix))]
    #[test]
    fn created_file_has_an_owner() {
        let dir = crate::util::test_dir("details-owner");
        let path = dir.join("owned");
        std::fs::write(&path, b"").unwrap();
        assert!(!owner(&path).is_empty());
        assert_eq!(owner(&path), owner(&path));
    }
    /// A SID no account answers to reads as the SID, not as an empty cell.
    #[cfg(windows)]
    #[test]
    fn a_sid_with_no_account_reads_as_the_sid() {
        use windows::Win32::Foundation::{LocalFree, HLOCAL};
        use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
        use windows::Win32::Security::PSID;
        let label = |s: &str| unsafe {
            let wide: Vec<u16> = s.encode_utf16().chain(Some(0)).collect();
            let mut sid = PSID::default();
            ConvertStringSidToSidW(windows::core::PCWSTR(wide.as_ptr()), &mut sid).unwrap();
            let out = sid_label(sid);
            let _ = LocalFree(Some(HLOCAL(sid.0)));
            out
        };
        let system = label("S-1-5-18");
        assert!(!system.is_empty() && !system.starts_with("S-1-"), "a known account keeps its name: {system}");
        assert_eq!(label("S-1-5-21-1-2-3-1001"), "S-1-5-21-1-2-3-1001");
    }
}
