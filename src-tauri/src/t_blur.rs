// RP-CUSTOM: Blur detection scan + PromptFix AI-fix batch; entire file is fork-only.
use crate::t_dedup;
use crate::t_sqlite::{AFile, QueryParams};
use reqwest::multipart;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Emitter;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlurScanStatus {
    pub state: String,
    pub phase: String,
    pub scope_key: String,
    pub current: u64,
    pub total: u64,
    pub blurry_count: u64,
    pub is_scanning: bool,
}

impl Default for BlurScanStatus {
    fn default() -> Self {
        Self {
            state: "idle".into(),
            phase: "idle".into(),
            scope_key: String::new(),
            current: 0,
            total: 0,
            blurry_count: 0,
            is_scanning: false,
        }
    }
}

#[derive(Default)]
pub struct BlurState {
    pub is_scanning: Arc<AtomicBool>,
    pub cancel_flag: Arc<AtomicBool>,
    pub status: Arc<Mutex<BlurScanStatus>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlurFixStatus {
    pub state: String,
    pub current: u64,
    pub total: u64,
    pub current_file: String,
    pub is_fixing: bool,
    pub error: Option<String>,
}

impl Default for BlurFixStatus {
    fn default() -> Self {
        Self {
            state: "idle".into(),
            current: 0,
            total: 0,
            current_file: String::new(),
            is_fixing: false,
            error: None,
        }
    }
}

#[derive(Default)]
pub struct BlurFixState {
    pub is_fixing: Arc<AtomicBool>,
    pub cancel_flag: Arc<AtomicBool>,
    pub status: Arc<Mutex<BlurFixStatus>>,
}

fn get_db_conn() -> Result<Connection, String> {
    let path = crate::t_storage::get_current_db_path().map_err(|e| e.to_string())?;
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn resolve_scope(
    params: Option<QueryParams>,
    collection_id: Option<i64>,
    file_ids: Option<Vec<i64>>,
) -> Result<Vec<AFile>, String> {
    if file_ids.is_some() && (params.is_some() || collection_id.is_some()) {
        return Err("File ID scope cannot be combined with query or collection scope.".into());
    }
    if let Some(ids) = file_ids {
        AFile::get_files_by_ids(&ids)
    } else if let Some(id) = collection_id {
        t_dedup::get_files_by_collection(id, params.as_ref())
    } else {
        t_dedup::get_files_by_query(params.as_ref().ok_or("Query required")?)
    }
}

/// Laplacian-variance score below which a photo is flagged as blurry.
/// Calibrated for `score_file`: scored at <=1024px with a light Gaussian pre-blur,
/// where the blurry test photos land at ~2-10 and ordinary sharp photos at ~20-80.
/// Very flat/hazy shots can also fall below it, so results are for human review.
// RP-CUSTOM: photo-blur scale differs from t_common::MIN_BLUR_SCORE (200, face-quality blur on full-res crops); don't unify.
pub const BLUR_THRESHOLD: f32 = 12.0;
/// Longest side (px) every image is scaled down to before scoring, so scores are
/// comparable across resolutions.
const SCORE_LONG_SIDE: u32 = 1024;
/// Gaussian sigma applied before the Laplacian to suppress sensor noise / JPEG grain.
const SCORE_PRE_BLUR_SIGMA: f32 = 1.0;

/// Decode a reduced-size copy of the file via the thumbnail pipeline (fast scaled JPEG
/// decode, plus HEIC/JXL/RAW support) and fall back to a full decode.
fn load_scoring_image(path: &str, file_type: i64) -> Option<image::DynamicImage> {
    let thumb = if file_type == 3 {
        crate::t_image::get_raw_thumbnail(
            path,
            1,
            SCORE_LONG_SIDE,
            crate::t_raw_display::RawDisplayOptions::rendered_bright(),
        )
    } else {
        crate::t_image::get_image_thumbnail(path, 1, SCORE_LONG_SIDE)
    };
    if let Ok(Some(bytes)) = thumb {
        if let Ok(img) = image::load_from_memory(&bytes) {
            return Some(img);
        }
    }
    image::open(Path::new(path)).ok()
}

/// Resolution-normalised, noise-suppressed Laplacian variance. Higher = sharper.
fn score_file(path: &str, file_type: i64) -> Option<f32> {
    let mut img = load_scoring_image(path, file_type)?;
    if img.width().max(img.height()) > SCORE_LONG_SIDE {
        img = img.resize(
            SCORE_LONG_SIDE,
            SCORE_LONG_SIDE,
            image::imageops::FilterType::Triangle,
        );
    }
    let img = image::DynamicImage::ImageLuma8(img.to_luma8()).blur(SCORE_PRE_BLUR_SIGMA);
    Some(crate::t_common::laplacian_variance(&img, 0, 0, img.width(), img.height()))
}

pub fn start_scan(
    app: tauri::AppHandle,
    state: tauri::State<'_, BlurState>,
    scope_key: String,
    source_version: i64,
    params: Option<QueryParams>,
    collection_id: Option<i64>,
    file_ids: Option<Vec<i64>>,
) -> Result<(), String> {
    if state
        .is_scanning
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A blur scan is already running.".into());
    }
    state.cancel_flag.store(false, Ordering::SeqCst);
    *state.status.lock().unwrap() = BlurScanStatus {
        state: "running".into(),
        phase: "preparing".into(),
        scope_key: scope_key.clone(),
        current: 0,
        total: 0,
        blurry_count: 0,
        is_scanning: true,
    };
    let status = state.status.clone();
    let running = state.is_scanning.clone();
    let cancel = state.cancel_flag.clone();
    std::thread::spawn(move || {
        let result = scan(
            &app,
            &status,
            &cancel,
            &scope_key,
            source_version,
            params,
            collection_id,
            file_ids,
        );
        let mut current = status.lock().unwrap();
        current.state = if result.is_ok() && !cancel.load(Ordering::SeqCst) {
            "finished".into()
        } else if cancel.load(Ordering::SeqCst) {
            "idle".into()
        } else {
            "error".into()
        };
        current.is_scanning = false;
        running.store(false, Ordering::SeqCst);
        let _ = app.emit("blur-scan-progress", current.clone());
    });
    Ok(())
}

pub fn cancel_scan(state: tauri::State<'_, BlurState>) {
    state.cancel_flag.store(true, Ordering::SeqCst);
}

pub fn get_status(state: tauri::State<'_, BlurState>) -> BlurScanStatus {
    state.status.lock().unwrap().clone()
}

fn scan(
    app: &tauri::AppHandle,
    status: &Arc<Mutex<BlurScanStatus>>,
    cancel: &Arc<AtomicBool>,
    scope_key: &str,
    source_version: i64,
    params: Option<QueryParams>,
    collection_id: Option<i64>,
    file_ids: Option<Vec<i64>>,
) -> Result<(), String> {
    let mut conn = get_db_conn()?;
    let images: Vec<AFile> = resolve_scope(params, collection_id, file_ids)?
        .into_iter()
        .filter(|f| matches!(f.file_type, Some(1) | Some(3)) && f.id.is_some())
        .collect();

    {
        let mut s = status.lock().unwrap();
        s.total = images.len() as u64;
        s.current = 0;
        s.phase = "scanning".into();
    }
    let _ = app.emit("blur-scan-progress", status.lock().unwrap().clone());

    let mut results: Vec<(i64, f32, bool)> = Vec::new();
    for (i, file) in images.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }
        let Some(file_id) = file.id else { continue };
        let Some(path) = file.file_path.as_ref() else {
            continue;
        };
        let Some(score) = score_file(path, file.file_type.unwrap_or(1)) else {
            continue;
        };
        let is_blurry = score < BLUR_THRESHOLD;
        results.push((file_id, score, is_blurry));

        if i % 20 == 0 {
            let mut s = status.lock().unwrap();
            s.current = i as u64;
            let _ = app.emit("blur-scan-progress", s.clone());
        }
    }
    if cancel.load(Ordering::SeqCst) {
        return Ok(());
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let blurry_count = results.iter().filter(|(_, _, is_blurry)| *is_blurry).count();

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM blur_scans WHERE scope_key=?1",
        params![scope_key],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO blur_scans(scope_key,source_version,status,file_count,blurry_count,created_at,completed_at) VALUES(?1,?2,'finished',?3,?4,?5,?5)",
        params![scope_key, source_version, results.len() as i64, blurry_count as i64, now],
    )
    .map_err(|e| e.to_string())?;
    let scan_id = tx.last_insert_rowid();
    for (file_id, score, is_blurry) in &results {
        tx.execute(
            "INSERT INTO blur_results(scan_id,file_id,blur_score,is_blurry) VALUES(?1,?2,?3,?4)",
            params![scan_id, file_id, *score, *is_blurry as i64],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;

    let mut s = status.lock().unwrap();
    s.phase = "finished".into();
    s.current = s.total;
    s.blurry_count = blurry_count as u64;
    Ok(())
}

pub fn list_results(
    scope_key: &str,
    limit: i64,
    offset: i64,
) -> Result<serde_json::Value, String> {
    let conn = get_db_conn()?;
    let total = conn
        .query_row(
            "SELECT COUNT(*) FROM blur_results r JOIN blur_scans s ON s.id=r.scan_id WHERE s.scope_key=?1 AND r.is_blurry=1",
            params![scope_key],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT r.file_id, r.blur_score, r.fixed_path, r.fixed_at FROM blur_results r JOIN blur_scans s ON s.id=r.scan_id WHERE s.scope_key=?1 AND r.is_blurry=1 ORDER BY r.blur_score ASC LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![scope_key, limit.max(1), offset.max(0)], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, f32>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let (file_id, blur_score, fixed_path, fixed_at) = row.map_err(|e| e.to_string())?;
        let Some(file) = AFile::get_file_info(file_id)? else {
            continue;
        };
        out.push(serde_json::json!({
            "file_id": file_id,
            "blur_score": blur_score,
            "fixed_path": fixed_path,
            "fixed_at": fixed_at,
            "file": file,
        }));
    }
    Ok(serde_json::json!({"items": out, "total": total}))
}

pub fn has_scan(scope_key: &str) -> Result<bool, String> {
    let conn = get_db_conn()?;
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM blur_scans WHERE scope_key=?1)",
        params![scope_key],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

/// Derive a sibling "<name>_fixed.<ext>" path next to the original file.
fn fixed_output_path(original: &str) -> String {
    let path = Path::new(original);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "fixed".to_string());
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let file_name = format!("{stem}_fixed.{ext}");
    match path.parent() {
        Some(parent) => parent.join(file_name).to_string_lossy().to_string(),
        None => file_name,
    }
}

pub async fn start_fix_batch(
    app: tauri::AppHandle,
    state: tauri::State<'_, BlurFixState>,
    file_ids: Vec<i64>,
    api_base_url: String,
    task: Option<String>,
    instruction: Option<String>,
) -> Result<(), String> {
    if task.is_none() && instruction.is_none() {
        return Err("Provide either a task or a custom instruction.".into());
    }
    if state
        .is_fixing
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("An AI-fix batch is already running.".into());
    }
    state.cancel_flag.store(false, Ordering::SeqCst);
    *state.status.lock().unwrap() = BlurFixStatus {
        state: "running".into(),
        current: 0,
        total: file_ids.len() as u64,
        current_file: String::new(),
        is_fixing: true,
        error: None,
    };
    let status = state.status.clone();
    let running = state.is_fixing.clone();
    let cancel = state.cancel_flag.clone();
    let app_for_task = app.clone();
    tokio::spawn(async move {
        let result = fix_batch(
            &app_for_task,
            &status,
            &cancel,
            file_ids,
            &api_base_url,
            task.as_deref(),
            instruction.as_deref(),
        )
        .await;
        let mut current = status.lock().unwrap();
        current.is_fixing = false;
        current.state = if cancel.load(Ordering::SeqCst) {
            "idle".into()
        } else if let Err(e) = &result {
            current.error = Some(e.clone());
            "error".into()
        } else {
            "finished".into()
        };
        running.store(false, Ordering::SeqCst);
        let _ = app_for_task.emit("blur-fix-progress", current.clone());
    });
    Ok(())
}

pub fn cancel_fix_batch(state: tauri::State<'_, BlurFixState>) {
    state.cancel_flag.store(true, Ordering::SeqCst);
}

pub fn get_fix_status(state: tauri::State<'_, BlurFixState>) -> BlurFixStatus {
    state.status.lock().unwrap().clone()
}

async fn fix_batch(
    app: &tauri::AppHandle,
    status: &Arc<Mutex<BlurFixStatus>>,
    cancel: &Arc<AtomicBool>,
    file_ids: Vec<i64>,
    api_base_url: &str,
    task: Option<&str>,
    instruction: Option<&str>,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    client
        .get(format!("{api_base_url}/health"))
        .send()
        .await
        .map_err(|_| {
            format!(
                "Could not reach the PromptFix service at {api_base_url}. Start it first (see promptfix_service/README.md)."
            )
        })?;

    for (i, file_id) in file_ids.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Ok(());
        }
        let Some(file) = AFile::get_file_info(*file_id)? else {
            continue;
        };
        let Some(path) = file.file_path.clone() else {
            continue;
        };

        {
            let mut s = status.lock().unwrap();
            s.current = i as u64;
            s.current_file = file.name.clone();
            let _ = app.emit("blur-fix-progress", s.clone());
        }

        let bytes = std::fs::read(&path).map_err(|e| format!("Failed to read {path}: {e}"))?;
        let part = multipart::Part::bytes(bytes).file_name(file.name.clone());
        let mut form = multipart::Form::new().part("file", part);
        if let Some(task) = task {
            form = form.text("task", task.to_string());
        } else if let Some(instruction) = instruction {
            form = form.text("instruction", instruction.to_string());
        }
        let response = client
            .post(format!("{api_base_url}/fix"))
            .multipart(form)
            .send()
            .await
            .map_err(|_| {
                format!("Could not reach the PromptFix service at {api_base_url}. Start it first.")
            })?;

        if !response.status().is_success() {
            // Skip files the service can't process; keep the batch going.
            continue;
        }
        let fixed_bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read response for {path}: {e}"))?;

        let output_path = fixed_output_path(&path);
        std::fs::write(&output_path, &fixed_bytes)
            .map_err(|e| format!("Failed to write {output_path}: {e}"))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let conn = get_db_conn()?;
        conn.execute(
            "UPDATE blur_results SET fixed_path=?1, fixed_at=?2 WHERE file_id=?3",
            params![output_path, now, file_id],
        )
        .map_err(|e| e.to_string())?;
    }

    let mut s = status.lock().unwrap();
    s.current = s.total;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{imageops, Rgb, RgbImage};

    fn textured(w: u32, h: u32) -> RgbImage {
        // Deterministic high-detail pattern (edges at many scales).
        RgbImage::from_fn(w, h, |x, y| {
            let v = ((x * 7 + y * 13) % 251) as u8 ^ (((x / 3) * (y / 5)) % 256) as u8;
            Rgb([v, v, v])
        })
    }

    fn score_of(img: &RgbImage, name: &str) -> f32 {
        let path = std::env::temp_dir().join(name);
        img.save(&path).unwrap();
        let s = score_file(path.to_str().unwrap(), 1).unwrap();
        let _ = std::fs::remove_file(&path);
        s
    }

    #[test]
    fn sharp_scores_above_threshold_and_blurred_below() {
        let sharp = textured(2400, 1600);
        let blurred = imageops::blur(&sharp, 6.0);
        let s = score_of(&sharp, "lap_blur_test_sharp.jpg");
        let b = score_of(&blurred, "lap_blur_test_blurred.jpg");
        assert!(s > BLUR_THRESHOLD, "sharp scored {s}");
        assert!(b < BLUR_THRESHOLD, "blurred scored {b}");
    }

    #[test]
    fn score_is_resolution_independent() {
        let small = textured(1000, 700);
        let big = imageops::resize(&small, 3000, 2100, imageops::FilterType::Triangle);
        let a = score_of(&small, "lap_blur_test_small.png");
        let b = score_of(&big, "lap_blur_test_big.png");
        // Same content upscaled should be classified identically (not 9x apart).
        assert_eq!(a < BLUR_THRESHOLD, b < BLUR_THRESHOLD, "{a} vs {b}");
    }

    #[test]
    #[ignore]
    fn print_scores_for_folder() {
        let dir = std::env::var("BLUR_DIR").unwrap();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if let Some(s) = score_file(p.to_str().unwrap(), 1) {
                println!("{:?}: {s:.1} blurry={}", p.file_name().unwrap(), s < BLUR_THRESHOLD);
            }
        }
    }
}
