/**
 * Common constants and shared types
 */

// Image support
pub const NORMAL_IMGS: &[&str] = &[
    "jpg", "jpeg", "jfif", "png", "gif", "bmp", "tif", "tiff", "webp", "avif", "heic", "heif", "hif", "jxl",
];

pub const AFFINITY_IMGS: &[&str] = &["af", "afphoto", "afdesign", "afpub"];

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
    "mos", "iiq", // Leaf / Phase One
    // "x3f", // Sigma / Foveon - temporarily disabled: current LibRaw path reports FileUnsupported for sampled X3F files, so indexing fails at RAW dimensions.
    "dcr", "kdc", // Kodak
    "erf", // Epson
    "mef", // Mamiya
    "raw", // Generic vendor RAW extension
    "mdc", // Legacy RAW variant in sample set
];

// Video support
pub const VIDEOS: &[&str] = &[
    "mpg", "mpeg", "vob", "mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "3gp", "m4v", "hevc", "asf",
    "mts", "m2ts", "mod", "tod", "ts",
];
// pub const AUDIOS: &[&str] = &[
//     "mp3", "wav", "flac", "aac", "m4a", "ogg", "wma", "mp2", "mp1", "ape", "alac", "wavpack",
// ];

/// Human-readable description of a file format extension.
pub(crate) fn extension_description(ext: &str) -> &'static str {
    match ext {
        "3fr" => "Hasselblad RAW",
        "3gp" => "3GPP Multimedia",
        "arw" => "Sony Alpha RAW",
        "asf" => "Advanced Systems Format",
        "avi" => "Audio Video Interleave",
        "avif" => "AV1 Image File Format",
        "bmp" => "Bitmap Image File",
        "cr2" => "Canon RAW 2",
        "cr3" => "Canon RAW 3",
        "crw" => "Canon RAW",
        "dcr" => "Kodak RAW",
        "dds" => "DirectDraw Surface",
        "dng" => "Digital Negative",
        "dpx" => "Digital Picture Exchange",
        "erf" => "Epson RAW",
        "exr" => "OpenEXR High Dynamic Range Image",
        "flv" => "Flash Video",
        "gif" => "Graphics Interchange Format",
        "hdr" => "Radiance High Dynamic Range Image",
        "heic" => "High Efficiency Image Container",
        "heif" => "High Efficiency Image File",
        "hevc" => "High Efficiency Video Coding",
        "hif" => "HEIF Image File",
        "j2c" => "JPEG 2000 Codestream",
        "j2k" => "JPEG 2000 Codestream",
        "jp2" => "JPEG 2000 Image",
        "jpc" => "JPEG 2000 Codestream",
        "jpeg" => "Joint Photographic Experts Group",
        "jfif" => "JPEG File Interchange Format",
        "jpf" => "JPEG 2000 Image",
        "jpg" => "Joint Photographic Experts Group",
        "jpx" => "JPEG 2000 Extended Image",
        "jxl" => "JPEG XL",
        "kdc" => "Kodak Digital Camera RAW",
        "m2ts" => "Blu-ray BDAV Video",
        "m4v" => "MPEG-4 Video",
        "mdc" => "Minolta RAW",
        "mef" => "Mamiya RAW",
        "mkv" => "Matroska Video",
        "mod" => "Camcorder Video",
        "mos" => "Leaf RAW",
        "mov" => "QuickTime Movie",
        "mp4" => "MPEG-4 Video",
        "mpeg" => "Moving Picture Experts Group Video",
        "mpg" => "MPEG Video",
        "mrw" => "Minolta RAW",
        "mts" => "AVCHD Video",
        "nef" => "Nikon Electronic Format",
        "nrw" => "Nikon RAW",
        "orf" => "Olympus RAW Format",
        "pef" => "Pentax Electronic File",
        "png" => "Portable Network Graphics",
        "af" | "afphoto" | "afdesign" | "afpub" => "Affinity Document",
        "psd" => "Photoshop Document",
        "qoi" => "Quite OK Image",
        "raf" => "Fujifilm RAW",
        "raw" => "Generic Camera RAW",
        "rgbe" => "Radiance RGBE Image",
        "rw2" => "Panasonic RAW 2",
        "rwl" => "Leica RAW",
        "sr2" => "Sony RAW 2",
        "srf" => "Sony RAW Format",
        "srw" => "Samsung RAW",
        "tga" => "Truevision Targa Image",
        "tif" => "Tagged Image File",
        "tiff" => "Tagged Image File Format",
        "tod" => "JVC HD Camcorder Video",
        "ts" => "MPEG Transport Stream",
        "vob" => "DVD Video Object",
        "webm" => "WebM Video",
        "webp" => "Web Picture Format",
        "wmv" => "Windows Media Video",
        _ => "File Format",
    }
}

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
