// RP-CUSTOM: Send-to-Smart-Photos feature; entire file is fork-only.
//! "Send to Smart Photos": copy selected photos into a curated
//! `<root>\<year>\<event folder>\photo.jpg` tree. See docs/smart-photos-design.md.
//!
//! SOURCE IS STRICTLY READ-ONLY. Source files are only ever opened for reading;
//! nothing here renames, moves, deletes or re-stamps a source path. All writes
//! (directory creation, copy, timestamps, overwrite) target the Smart Photos root.

use chrono::{DateTime, Datelike, Local};
use exif::{In, Reader, Tag};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

/// Modified-time tolerance for the "already exists" check (exFAT stores 2 s resolution).
const MTIME_TOLERANCE: Duration = Duration::from_secs(2);

#[derive(Serialize, Default, Debug, PartialEq)]
pub struct SendResult {
    pub copied: usize,
    pub skipped_exists: usize,
    pub skipped_layout: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[derive(Serialize, Default, Debug, PartialEq)]
pub struct Preflight {
    /// Supported (jpg/jpeg/png) files found in the selection.
    pub total: usize,
    /// Files whose folder is not laid out as `<year>\<event folder>`.
    pub needs_fallback: usize,
}

fn is_supported(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("jpg" | "jpeg" | "png")
    )
}

/// Expands files and folders into the supported files they contain (de-duplicated).
/// RAW files, videos and anything else are ignored without comment.
fn collect_files(paths: &[String]) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut push = |p: PathBuf| {
        if is_supported(&p) && seen.insert(p.clone()) {
            out.push(p);
        }
    };
    for raw in paths {
        let p = PathBuf::from(raw);
        if p.is_dir() {
            for entry in WalkDir::new(&p).into_iter().filter_map(Result::ok) {
                if entry.file_type().is_file() {
                    push(entry.into_path());
                }
            }
        } else if p.is_file() {
            push(p);
        }
    }
    out
}

fn parse_year(s: &str) -> Option<String> {
    if s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit()) {
        let y: u32 = s.parse().ok()?;
        if (1800..=2200).contains(&y) {
            return Some(s.to_string());
        }
    }
    None
}

/// ASSUMPTION: the source library is laid out as <year>\<event folder>\photo.jpg.
/// The year is therefore taken from the name of the event folder's parent folder.
/// If the layout differs, the caller must ask the user before falling back to the
/// photo's EXIF date or the file's modified date (see `year_from_date`).
fn year_from_layout(source: &Path) -> Option<String> {
    let year_dir = source.parent()?.parent()?;
    parse_year(year_dir.file_name()?.to_str()?)
}

/// Fallback year: EXIF DateTimeOriginal, then EXIF DateTime, then file modified date.
fn year_from_date(source: &Path) -> Option<String> {
    if let Ok(file) = fs::File::open(source) {
        let mut reader = std::io::BufReader::new(file);
        if let Ok(exif) = Reader::new().read_from_container(&mut reader) {
            for tag in [Tag::DateTimeOriginal, Tag::DateTime] {
                if let Some(field) = exif.get_field(tag, In::PRIMARY) {
                    let text = field.display_value().to_string();
                    if let Some(year) = text.get(..4).and_then(parse_year) {
                        return Some(year);
                    }
                }
            }
        }
    }
    let modified = fs::metadata(source).ok()?.modified().ok()?;
    parse_year(&DateTime::<Local>::from(modified).year().to_string())
}

/// Destination for `source`, or None when no year can be determined.
/// The year comes from the folder layout; the date fallback is only used when allowed.
fn resolve_destination(source: &Path, root: &Path, allow_date_fallback: bool) -> Option<PathBuf> {
    let event = source.parent()?.file_name()?;
    let year = year_from_layout(source)
        .or_else(|| if allow_date_fallback { year_from_date(source) } else { None })?;
    Some(root.join(year).join(event).join(source.file_name()?))
}

fn same_size_and_mtime(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    if a.len() != b.len() {
        return false;
    }
    match (a.modified(), b.modified()) {
        (Ok(x), Ok(y)) => {
            let diff = x.duration_since(y).or_else(|_| y.duration_since(x)).unwrap_or_default();
            diff <= MTIME_TOLERANCE
        }
        _ => false,
    }
}

fn is_same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

enum CopyOutcome {
    Copied,
    SkippedExists,
}

/// Copies `source` to `dest` (byte for byte, so EXIF is preserved), then gives the
/// copy the source's created/modified/accessed times. The source is only read.
fn copy_one(source: &Path, dest: &Path) -> Result<CopyOutcome, String> {
    crate::t_utils::require_local_content(source).map_err(|e| e.to_string())?;
    let src_meta = fs::metadata(source).map_err(|e| e.to_string())?;

    if let Ok(dst_meta) = fs::metadata(dest) {
        if is_same_file(source, dest) {
            return Err("source and destination are the same file".to_string());
        }
        if same_size_and_mtime(&src_meta, &dst_meta) {
            return Ok(CopyOutcome::SkippedExists);
        }
    }

    let dir = dest.parent().ok_or("invalid destination")?;
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {}", dir.display(), e))?;

    // Copy to a temp name beside the destination, stamp it, then swap it in, so a
    // failure never leaves a half-written or wrongly-dated file at the final name.
    let mut tmp_name = dest.file_name().ok_or("invalid destination")?.to_os_string();
    tmp_name.push(".lap-tmp");
    let tmp = dir.join(tmp_name);
    let result = (|| -> Result<(), String> {
        fs::copy(source, &tmp).map_err(|e| e.to_string())?;
        crate::t_image::restore_file_times(&tmp, &src_meta)?;
        if let Ok(meta) = fs::metadata(dest) {
            // An existing read-only destination would block the overwrite.
            let mut perms = meta.permissions();
            if perms.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                perms.set_readonly(false);
                let _ = fs::set_permissions(dest, perms);
            }
        }
        fs::rename(&tmp, dest).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map(|_| CopyOutcome::Copied)
}

/// "unset" | "missing" (e.g. drive not connected) | "readonly" | "ok"
fn root_status(root: &str) -> &'static str {
    let root = root.trim();
    if root.is_empty() {
        return "unset";
    }
    let path = Path::new(root);
    if !path.is_dir() {
        return "missing";
    }
    let probe = path.join(".lap-write-test");
    match fs::File::create(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            "ok"
        }
        Err(_) => "readonly",
    }
}

fn send_files(files: &[PathBuf], root: &Path, allow_date_fallback: bool) -> SendResult {
    let mut res = SendResult::default();
    for source in files {
        let Some(dest) = resolve_destination(source, root, allow_date_fallback) else {
            res.skipped_layout += 1;
            continue;
        };
        match copy_one(source, &dest) {
            Ok(CopyOutcome::Copied) => res.copied += 1,
            Ok(CopyOutcome::SkippedExists) => res.skipped_exists += 1,
            Err(e) => {
                res.failed += 1;
                res.errors.push(format!("{}: {}", source.display(), e));
            }
        }
    }
    res
}

#[tauri::command]
pub async fn smart_photos_root_status(root: String) -> String {
    tauri::async_runtime::spawn_blocking(move || root_status(&root).to_string())
        .await
        .unwrap_or_else(|_| "missing".to_string())
}

/// Counts supported files and how many would need the date fallback.
#[tauri::command]
pub async fn smart_photos_preflight(paths: Vec<String>) -> Result<Preflight, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let files = collect_files(&paths);
        let needs_fallback = files.iter().filter(|f| year_from_layout(f).is_none()).count();
        Preflight { total: files.len(), needs_fallback }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_to_smart_photos(
    paths: Vec<String>,
    root: String,
    allow_date_fallback: bool,
) -> Result<SendResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        match root_status(&root) {
            "ok" => {}
            "unset" => return Err("Smart Photos folder is not set".to_string()),
            "readonly" => return Err("Smart Photos folder is not writable".to_string()),
            _ => return Err("Smart Photos folder is not available (is the drive connected?)".to_string()),
        }
        let files = collect_files(&paths);
        Ok(send_files(&files, Path::new(root.trim()), allow_date_fallback))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// For each input file (no folder expansion): is an identical copy already in Smart Photos?
/// Returns all-false when the root is unavailable, so an offline drive shows no badges.
#[tauri::command]
pub async fn check_in_smart_photos(paths: Vec<String>, root: String) -> Vec<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        if root_status(&root) == "unset" || !Path::new(root.trim()).is_dir() {
            return vec![false; paths.len()];
        }
        let root = Path::new(root.trim());
        paths
            .iter()
            .map(|p| {
                let source = Path::new(p);
                if !is_supported(source) {
                    return false;
                }
                let Some(dest) = resolve_destination(source, root, true) else { return false };
                match (fs::metadata(source), fs::metadata(&dest)) {
                    (Ok(a), Ok(b)) => !is_same_file(source, &dest) && same_size_and_mtime(&a, &b),
                    _ => false,
                }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lap-sp-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn set_mtime(path: &Path, t: SystemTime) {
        fs::OpenOptions::new().write(true).open(path).unwrap()
            .set_times(fs::FileTimes::new().set_modified(t)).unwrap();
    }

    #[test]
    fn year_comes_from_parent_of_event_folder_and_dirs_are_created() {
        let base = temp_dir("layout");
        let src = base.join("lib/2010/2010_01_01/a.jpg");
        write(&src, b"jpegdata");
        let root = base.join("smart");
        fs::create_dir_all(&root).unwrap();

        let res = send_files(&[src.clone()], &root, false);
        assert_eq!(res.copied, 1);
        assert_eq!(fs::read(root.join("2010/2010_01_01/a.jpg")).unwrap(), b"jpegdata");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn layout_mismatch_is_skipped_without_fallback_and_uses_mtime_with_it() {
        let base = temp_dir("fallback");
        let src = base.join("lib/holiday/a.png");
        write(&src, b"png");
        let when = SystemTime::UNIX_EPOCH + Duration::from_secs(1_262_304_000 + 86_400 * 30); // Jan 2010
        set_mtime(&src, when);
        let root = base.join("smart");
        fs::create_dir_all(&root).unwrap();

        let res = send_files(&[src.clone()], &root, false);
        assert_eq!((res.copied, res.skipped_layout), (0, 1));
        assert!(!root.join("2010").exists());

        let res = send_files(&[src.clone()], &root, true);
        assert_eq!(res.copied, 1);
        assert!(root.join("2010/holiday/a.png").exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn folders_are_expanded_and_only_jpg_jpeg_png_are_taken() {
        let base = temp_dir("collect");
        for n in ["a.jpg", "b.JPEG", "c.png", "d.cr2", "e.mp4", "f.txt"] {
            write(&base.join("2011/2011_02_02").join(n), b"x");
        }
        let files = collect_files(&[base.join("2011").to_string_lossy().to_string()]);
        let mut names: Vec<_> = files.iter().map(|f| f.file_name().unwrap().to_string_lossy().to_string()).collect();
        names.sort();
        assert_eq!(names, ["a.jpg", "b.JPEG", "c.png"]);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn copy_preserves_bytes_and_times_and_never_touches_source() {
        let base = temp_dir("preserve");
        let src = base.join("lib/2012/2012_03_03/a.jpg");
        write(&src, b"original-bytes");
        let when = SystemTime::now() - Duration::from_secs(86_400 * 400);
        set_mtime(&src, when);
        let before = fs::metadata(&src).unwrap();
        let root = base.join("smart");
        fs::create_dir_all(&root).unwrap();

        send_files(&[src.clone()], &root, false);
        let dest = root.join("2012/2012_03_03/a.jpg");
        assert_eq!(fs::read(&dest).unwrap(), b"original-bytes");
        let dm = fs::metadata(&dest).unwrap();
        assert!(same_size_and_mtime(&before, &dm));

        let after = fs::metadata(&src).unwrap();
        assert_eq!(fs::read(&src).unwrap(), b"original-bytes");
        assert_eq!(before.modified().unwrap(), after.modified().unwrap());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn identical_is_skipped_and_changed_is_overwritten() {
        let base = temp_dir("collide");
        let src = base.join("lib/2013/2013_04_04/a.jpg");
        write(&src, b"v1");
        let root = base.join("smart");
        fs::create_dir_all(&root).unwrap();

        assert_eq!(send_files(&[src.clone()], &root, false).copied, 1);
        assert_eq!(send_files(&[src.clone()], &root, false).skipped_exists, 1);

        write(&src, b"version-two");
        let r = send_files(&[src.clone()], &root, false);
        assert_eq!(r.copied, 1);
        assert_eq!(fs::read(root.join("2013/2013_04_04/a.jpg")).unwrap(), b"version-two");
        assert_eq!(fs::read(&src).unwrap(), b"version-two");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn root_equal_to_library_never_overwrites_the_source() {
        let base = temp_dir("same");
        let src = base.join("2014/2014_05_05/a.jpg");
        write(&src, b"keep-me");
        let r = send_files(&[src.clone()], &base, false);
        assert_eq!((r.copied, r.failed), (0, 1));
        assert_eq!(fs::read(&src).unwrap(), b"keep-me");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn root_status_reports_unset_missing_ok() {
        assert_eq!(root_status(""), "unset");
        assert_eq!(root_status("Z:\\definitely\\not\\here"), "missing");
        let d = temp_dir("status");
        assert_eq!(root_status(&d.to_string_lossy()), "ok");
        let _ = fs::remove_dir_all(&d);
    }
}
