/**
 * Common constants and shared types
 */

// Image support
pub const NORMAL_IMGS: &[&str] = &[
    "jpg", "jpeg", "jfif", "png", "gif", "bmp", "tif", "tiff", "webp", "avif", "heic", "heif", "hif", "jxl",
];

// Image formats decoded through the bundled FFmpeg sidecar.
pub const FFMPEG_BACKED_IMGS: &[&str] = &[
    "psd", // Photoshop native format; preview only (merged composite), no edit support
    "exr", // HDR industry standard (VFX/rendering/Resolve output)
    "hdr", "rgbe", // Radiance RGBE format; HDR panoramas and tonemapped output
    "tga",  // Legacy format; some cameras/scanners/game asset pipelines
    "dds",  // DirectDraw Surface; game texture format; relevant only for game-dev users
    "qoi",  // Fast lossless format (2021); ecosystem still immature
    "jp2", "j2k", "j2c", "jpc", "jpf", "jpx", // JPEG 2000 family; medical/satellite use only
    "dpx", // Digital cinema intermediate format; niche film/grading pipeline only
];

// RAW support
pub const RAW_IMGS: &[&str] = &[
    "cr2", "cr3", "crw", // Canon
    "nef", "nrw", // Nikon
    "arw", "srf", "sr2", // Sony
    "raf", // Fujifilm
    "rw2", // Panasonic
    "orf", // Olympus / OM System
    "pef", // Pentax
    "dng", // Adobe / generic RAW
    "srw", // Samsung
    "rwl", // Leica
    "mrw", // Minolta / Konica Minolta
    "3fr", // Hasselblad
    "mos", // Leaf / Phase One
    // "x3f", // Sigma / Foveon - temporarily disabled: current LibRaw path reports FileUnsupported for sampled X3F files, so indexing fails at RAW dimensions.
    "dcr", "kdc", // Kodak
    "erf", // Epson
    "mef", // Mamiya
    "raw", // Generic vendor RAW extension
    "mdc", // Legacy RAW variant in sample set
];

// Video support
pub const VIDEOS: &[&str] = &[
    "mpg", "mpeg", "mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "3gp", "m4v", "hevc", "asf",
    "mts", "m2ts", "mod", "tod", "ts",
];
// pub const AUDIOS: &[&str] = &[
//     "mp3", "wav", "flac", "aac", "m4a", "ogg", "wma", "mp2", "mp1", "ape", "alac", "wavpack",
// ];

// AI search
pub const AI_TEXT_MODEL: &str = "text_model.onnx";
pub const AI_VISION_MODEL: &str = "vision_model.onnx";
pub const AI_TOKENIZER: &str = "tokenizer.json";

// Face Recognition Constants

// models
pub const DETECTION_MODEL: &str = "det_500m.onnx"; // RetinaFace
pub const EMBEDDING_MODEL: &str = "w600k_mbf.onnx"; // MobileFaceNet

// Quality thresholds - Recommended Values
pub const MIN_CONFIDENCE: f32 = 0.65; // 0.6-0.7 is standard. 0.65 balances precision/recall.
// pub const MIN_FACE_RATIO: f32 = 0.0; // Disabled. Rely on absolute pixel size instead (better for high-res photos).
// pub const MIN_FACE_SIZE: f32 = 90.0; // 80-112px is minimum for good recognition. 90.0 is a safe high-quality baseline.
pub const MIN_BLUR_SCORE: f32 = 200.0; // Standard Laplacian variance threshold. Below 100 is usually blurry.

/// Calculate the Laplacian variance (a standard blur metric) over a region of an image.
/// Optimized: Uses Welford's online algorithm to avoid allocating a large vector.
pub fn laplacian_variance(img: &image::DynamicImage, x: u32, y: u32, w: u32, h: u32) -> f32 {
    if w < 3 || h < 3 {
        return 0.0;
    }

    let crop = img.crop_imm(x, y, w, h).to_luma8();
    let (width, height) = crop.dimensions();

    // Online variance calculation (Welford's algorithm)
    let mut count = 0usize;
    let mut m2 = 0.0;
    let mut mean = 0.0;

    for cy in 1..height - 1 {
        for cx in 1..width - 1 {
            let p = crop.get_pixel(cx, cy).0[0] as i16;
            let top = crop.get_pixel(cx, cy - 1).0[0] as i16;
            let bottom = crop.get_pixel(cx, cy + 1).0[0] as i16;
            let left = crop.get_pixel(cx - 1, cy).0[0] as i16;
            let right = crop.get_pixel(cx + 1, cy).0[0] as i16;

            let sum = top + bottom + left + right - 4 * p;
            let val = sum as f32;

            count += 1;
            let delta = val - mean;
            mean += delta / count as f32;
            let delta2 = val - mean;
            m2 += delta * delta2;
        }
    }

    if count < 2 {
        return 0.0;
    }

    // Variance
    m2 / (count as f32)
}

// Clustering Constants
pub const K_NEIGHBORS: usize = 80; // Prune edges to Top-K (K-NN)
pub const MIN_SAMPLES: usize = 1; // Minimum samples per cluster
