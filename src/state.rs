use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SitePolicy {
    pub adblock: bool,
    pub js_enabled: bool,
    pub images_enabled: bool,
    pub desktop_ua: bool,
    pub oled_dark: bool,
    pub sponsorblock: bool,
    pub auto_reject_cookies: bool,
}

impl Default for SitePolicy {
    fn default() -> Self {
        Self {
            adblock: true,
            js_enabled: true,
            images_enabled: true,
            desktop_ua: false,
            oled_dark: false,
            sponsorblock: true,
            auto_reject_cookies: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BookmarkEntry {
    pub title: String,
    pub url: String,
    pub created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub title: String,
    pub url: String,
    pub visited_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedTab {
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub background_youtube_play: bool,
    pub strip_tracking_params: bool,
    pub block_popups: bool,
    pub default_threads: usize,
    pub speed_limit_bps: u64,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            background_youtube_play: true,
            strip_tracking_params: true,
            block_popups: true,
            default_threads: 16,
            speed_limit_bps: 0,
        }
    }
}

pub struct BrowserStorage;

impl BrowserStorage {
    pub fn app_data_dir() -> PathBuf {
        let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("ShunnoBrowser");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn downloads_root_dir() -> PathBuf {
        let base = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let dir = base.join("ShunnoBrowser");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn user_scripts_dir() -> PathBuf {
        let dir = Self::downloads_root_dir().join("scripts");
        let _ = fs::create_dir_all(&dir);
        let readme = dir.join("README_UserScripts.txt");
        if !readme.exists() {
            let _ = fs::write(
                readme,
                "Drop any .js user-script files into this folder.
                 Optional header: // @match example.com
                 Scripts matching the active domain run automatically on page load.",
            );
        }
        dir
    }

    pub fn load_matching_user_scripts(host: &str) -> String {
        let dir = Self::user_scripts_dir();
        let mut combined = String::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("js") {
                    if let Ok(code) = fs::read_to_string(&path) {
                        let matches = if let Some(match_line) = code
                            .lines()
                            .find(|l| l.trim_start().starts_with("// @match"))
                        {
                            let pattern = match_line
                                .trim_start()
                                .trim_start_matches("// @match")
                                .trim()
                                .trim_matches('*')
                                .trim_matches('/');
                            pattern.is_empty() || host.contains(pattern)
                        } else {
                            true
                        };
                        if matches {
                            combined.push_str("
try {
");
                            combined.push_str(&code);
                            combined.push_str("
} catch(e) { console.error('Shunno UserScript:', e); }
");
                        }
                    }
                }
            }
        }
        combined
    }

    pub fn load_global_settings() -> GlobalSettings {
        let path = Self::app_data_dir().join("global_settings.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_global_settings(settings: &GlobalSettings) {
        let path = Self::app_data_dir().join("global_settings.json");
        if let Ok(json) = serde_json::to_string_pretty(settings) {
            let _ = fs::write(path, json);
        }
    }

    pub fn load_site_policies() -> HashMap<String, SitePolicy> {
        let path = Self::app_data_dir().join("site_policies.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_site_policies(policies: &HashMap<String, SitePolicy>) {
        let path = Self::app_data_dir().join("site_policies.json");
        if let Ok(json) = serde_json::to_string_pretty(policies) {
            let _ = fs::write(path, json);
        }
    }

    pub fn load_bookmarks() -> Vec<BookmarkEntry> {
        let path = Self::app_data_dir().join("bookmarks.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| {
                vec![
                    BookmarkEntry {
                        title: "DuckDuckGo Privacy Search".to_string(),
                        url: "https://duckduckgo.com".to_string(),
                        created_at: 1,
                    },
                    BookmarkEntry {
                        title: "YouTube (Background Audio + SponsorBlock)".to_string(),
                        url: "https://www.youtube.com".to_string(),
                        created_at: 2,
                    },
                    BookmarkEntry {
                        title: "Internet Archive (Lossless FLAC & Media)".to_string(),
                        url: "https://archive.org".to_string(),
                        created_at: 3,
                    },
                ]
            })
    }

    pub fn save_bookmarks(bookmarks: &[BookmarkEntry]) {
        let path = Self::app_data_dir().join("bookmarks.json");
        if let Ok(json) = serde_json::to_string_pretty(bookmarks) {
            let _ = fs::write(path, json);
        }
    }

    pub fn export_portable_txt_bookmarks(bookmarks: &[BookmarkEntry]) -> PathBuf {
        let out_path = Self::downloads_root_dir().join("shunno_bookmarks.txt");
        let mut lines = Vec::with_capacity(bookmarks.len());
        for b in bookmarks {
            lines.push(format!("{} | {}", b.title.replace('|', "-"), b.url));
        }
        let _ = fs::write(&out_path, lines.join("
"));
        out_path
    }

    pub fn import_portable_txt_bookmarks(existing: &mut Vec<BookmarkEntry>) -> usize {
        let in_path = Self::downloads_root_dir().join("shunno_bookmarks.txt");
        let Ok(content) = fs::read_to_string(in_path) else {
            return 0;
        };
        let mut added = 0;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let (title, url) = if let Some((t, u)) = trimmed.split_once('|') {
                (t.trim().to_string(), u.trim().to_string())
            } else {
                (trimmed.to_string(), trimmed.to_string())
            };
            if url.starts_with("http") && !existing.iter().any(|b| b.url == url) {
                existing.insert(
                    0,
                    BookmarkEntry {
                        title,
                        url,
                        created_at: 0,
                    },
                );
                added += 1;
            }
        }
        if added > 0 {
            Self::save_bookmarks(existing);
        }
        added
    }

    pub fn load_history() -> Vec<HistoryEntry> {
        let path = Self::app_data_dir().join("history.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_history(history: &[HistoryEntry]) {
        let path = Self::app_data_dir().join("history.json");
        let capped: Vec<HistoryEntry> = history.iter().take(250).cloned().collect();
        if let Ok(json) = serde_json::to_string_pretty(&capped) {
            let _ = fs::write(path, json);
        }
    }

    pub fn load_session() -> Vec<SavedTab> {
        let path = Self::app_data_dir().join("session.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_session(tabs: &[SavedTab]) {
        let path = Self::app_data_dir().join("session.json");
        if let Ok(json) = serde_json::to_string_pretty(tabs) {
            let _ = fs::write(path, json);
        }
    }
}

pub fn sanitize_url(raw: &str, strip_tracking: bool) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "https://duckduckgo.com".to_string();
    }
    let normalized = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.contains('.') && !trimmed.contains(' ') {
        format!("https://{}", trimmed)
    } else {
        let encoded: String = trimmed
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                b' ' => "+".to_string(),
                _ => format!("%{:02X}", b),
            })
            .collect();
        format!("https://duckduckgo.com/?q={}", encoded)
    };

    if !strip_tracking {
        return normalized;
    }

    let Ok(mut parsed) = Url::parse(&normalized) else {
        return normalized;
    };

    let tracking_params = [
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_term",
        "utm_content",
        "fbclid",
        "gclid",
        "si",
        "igshid",
        "mc_cid",
        "mc_eid",
    ];

    let filtered: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(k, _)| !tracking_params.iter().any(|tp| k.eq_ignore_ascii_case(tp)))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if filtered.len() != parsed.query_pairs().count() {
        if filtered.is_empty() {
            parsed.set_query(None);
        } else {
            parsed.query_pairs_mut().clear().extend_pairs(filtered);
        }
    }

    parsed.to_string()
}

pub fn extract_host(url_str: &str) -> String {
    Url::parse(url_str)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .unwrap_or_default()
}
