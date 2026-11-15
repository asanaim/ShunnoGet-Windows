use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: u64,
    pub url: String,
    pub file_name: String,
    pub total_bytes: i64,
    pub downloaded_bytes: u64,
    pub status: String, // "QUEUED", "DOWNLOADING", "PAUSED", "COMPLETED", "FAILED"
    pub speed_bytes_per_sec: u64,
    pub thread_count: usize,
    pub category: String,
    pub supports_ranges: bool,
    pub referer: Option<String>,
    pub cookie: Option<String>,
    pub saved_file_path: Option<String>,
    pub sha256_hash: Option<String>,
    pub error_message: Option<String>,
    pub segment_map: String,
}

impl DownloadItem {
    pub fn progress_fraction(&self) -> f32 {
        if self.status == "COMPLETED" {
            1.0
        } else if self.total_bytes > 0 {
            ((self.downloaded_bytes as f64) / (self.total_bytes as f64)).clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    }

    pub fn badge_text(&self) -> String {
        let stream_label = if self.supports_ranges {
            format!("{} Threads", self.thread_count)
        } else {
            "1 Stream".to_string()
        };
        format!("{} • {} • {}", self.category, stream_label, self.status)
    }

    pub fn size_progress_text(&self) -> String {
        if self.total_bytes > 0 {
            let pct = (self.progress_fraction() * 100.0).clamp(0.0, 100.0);
            format!(
                "{} / {} ({:.1}%)",
                format_bytes(self.downloaded_bytes),
                format_bytes(self.total_bytes as u64),
                pct
            )
        } else {
            format_bytes(self.downloaded_bytes)
        }
    }

    pub fn telemetry_text(&self) -> String {
        match self.status.as_str() {
            "DOWNLOADING" => format!(
                "⚡ {}/s • ETA {} {}",
                format_bytes(self.speed_bytes_per_sec),
                format_eta(self.downloaded_bytes, self.total_bytes, self.speed_bytes_per_sec),
                self.segment_map
            ),
            "COMPLETED" => {
                if let Some(hash) = &self.sha256_hash {
                    let short = if hash.len() >= 16 { &hash[..16] } else { hash };
                    format!("✓ Saved • SHA-256: {}...", short)
                } else if let Some(path) = &self.saved_file_path {
                    format!("✓ Saved to {}", path)
                } else {
                    "✓ Completed".to_string()
                }
            }
            "FAILED" => format!(
                "⚠ {}",
                self.error_message
                    .as_deref()
                    .unwrap_or("Transfer interrupted")
            ),
            _ => format!("Paused at {}", format_bytes(self.downloaded_bytes)),
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let kb = bytes as f64 / 1024.0;
    let mb = kb / 1024.0;
    let gb = mb / 1024.0;
    if gb >= 1.0 {
        format!("{:.2} GB", gb)
    } else if mb >= 1.0 {
        format!("{:.1} MB", mb)
    } else if kb >= 1.0 {
        format!("{:.0} KB", kb)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_eta(downloaded: u64, total: i64, speed: u64) -> String {
    if total <= 0 || speed == 0 || downloaded >= total as u64 {
        return "--".to_string();
    }
    let remaining = (total as u64).saturating_sub(downloaded);
    let secs = remaining / speed;
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

pub fn detect_category(file_name: &str) -> String {
    let ext = file_name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "ts" | "m3u8" => "Videos".to_string(),
        "flac" | "alac" | "m4a" | "wav" | "mp3" | "ogg" | "opus" => "Lossless Audio".to_string(),
        "zip" | "rar" | "7z" | "tar" | "gz" | "iso" => "Archives".to_string(),
        "exe" | "msi" | "apk" => "Programs".to_string(),
        _ => "Documents".to_string(),
    }
}

pub struct QueueStorage;

impl QueueStorage {
    pub fn app_data_dir() -> PathBuf {
        let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("ShunnoGet");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn parts_dir() -> PathBuf {
        let dir = Self::app_data_dir().join("parts");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn target_folder(category: &str) -> PathBuf {
        let base = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let folder = base.join("ShunnoGet").join(category);
        let _ = fs::create_dir_all(&folder);
        folder
    }

    fn queue_file_path() -> PathBuf {
        Self::app_data_dir().join("shunnoget_queue.json")
    }

    pub fn load_queue() -> Vec<DownloadItem> {
        let path = Self::queue_file_path();
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(mut items) = serde_json::from_str::<Vec<DownloadItem>>(&data) {
                for item in &mut items {
                    if item.status == "DOWNLOADING" {
                        item.status = "PAUSED".to_string();
                        item.speed_bytes_per_sec = 0;
                    }
                }
                return items;
            }
        }
        Vec::new()
    }

    pub fn save_queue(items: &[DownloadItem]) {
        let path = Self::queue_file_path();
        if let Ok(json) = serde_json::to_string_pretty(items) {
            let _ = fs::write(path, json);
        }
    }
}
