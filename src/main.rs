#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod models;

use eframe::egui::{
    self, Color32, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke, Vec2, Visuals,
};
use engine::DownloadEngine;
use models::{detect_category, DownloadItem, QueueStorage};
use rand::Rng;
use regex::Regex;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

const SNIFF_EXTENSIONS: &[&str] = &[
    ".zip", ".7z", ".rar", ".exe", ".msi", ".iso", ".mp4", ".mkv", ".webm", ".m3u8", ".flac",
    ".m4a", ".wav", ".mp3", ".pdf", ".apk", ".tar", ".gz",
];

fn main() -> eframe::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to start Tokio runtime");

    let rt_handle = rt.handle().clone();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([980.0, 680.0])
            .with_min_inner_size([820.0, 560.0])
            .with_title("Shunno Get (Rust) — Multi-Threaded Download Manager"),
        ..Default::default()
    };

    eframe::run_native(
        "Shunno Get",
        native_options,
        Box::new(move |cc| Box::new(ShunnoGetApp::new(cc, rt_handle))),
    )
}

struct ShunnoGetApp {
    rt_handle: tokio::runtime::Handle,
    engine: DownloadEngine,
    queue: Arc<Mutex<Vec<DownloadItem>>>,
    url_input: String,
    selected_threads: usize,
    selected_category: String,
    speed_limit_idx: usize,
    clipboard_sniff_enabled: bool,
    last_clipboard_url: String,
    last_clip_check: Instant,
    footer_status: String,
    fix_link_target_id: Option<u64>,
    fix_link_input: String,
}

impl ShunnoGetApp {
    fn new(cc: &eframe::CreationContext<'_>, rt_handle: tokio::runtime::Handle) -> Self {
        let mut visuals = Visuals::light();
        visuals.panel_fill = Color32::from_rgb(242, 242, 247); // Apple #F2F2F7
        visuals.window_fill = Color32::WHITE;
        visuals.widgets.noninteractive.bg_fill = Color32::WHITE;
        visuals.widgets.inactive.rounding = Rounding::same(10.0);
        visuals.widgets.hovered.rounding = Rounding::same(10.0);
        visuals.widgets.active.rounding = Rounding::same(10.0);
        cc.egui_ctx.set_visuals(visuals);

        let initial_items = QueueStorage::load_queue();
        let queue = Arc::new(Mutex::new(initial_items));
        let engine = DownloadEngine::new();

        // Start local browser extension hook on http://127.0.0.1:45780/add?url=...
        start_browser_hook_server(&rt_handle, engine.clone(), queue.clone());

        Self {
            rt_handle,
            engine,
            queue,
            url_input: String::new(),
            selected_threads: 16,
            selected_category: "All".to_string(),
            speed_limit_idx: 0,
            clipboard_sniff_enabled: true,
            last_clipboard_url: String::new(),
            last_clip_check: Instant::now(),
            footer_status: "🦀 Rust + Tokio + Reqwest Engine Ready • Listening on http://127.0.0.1:45780/add"
                .to_string(),
            fix_link_target_id: None,
            fix_link_input: String::new(),
        }
    }

    fn enqueue_url(&mut self, raw_url: &str, referer: Option<String>, cookie: Option<String>) {
        let trimmed = raw_url.trim();
        let Ok(parsed) = Url::parse(trimmed) else {
            return;
        };
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return;
        }

        let raw_name = parsed
            .path_segments()
            .and_then(|s| s.last())
            .unwrap_or("download.bin");
        let file_name = if raw_name.contains('.') && !raw_name.is_empty() {
            raw_name.to_string()
        } else {
            format!("download_{}.bin", rand::thread_rng().gen_range(1000..9999))
        };

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1);
        let id = now_ms + rand::thread_rng().gen_range(1..999);

        let item = DownloadItem {
            id,
            url: parsed.to_string(),
            file_name: file_name.clone(),
            total_bytes: -1,
            downloaded_bytes: 0,
            status: "DOWNLOADING".to_string(),
            speed_bytes_per_sec: 0,
            thread_count: self.selected_threads,
            category: detect_category(&file_name),
            supports_ranges: true,
            referer,
            cookie,
            saved_file_path: None,
            sha256_hash: None,
            error_message: None,
            segment_map: String::new(),
        };

        if let Ok(mut list) = self.queue.lock() {
            list.insert(0, item.clone());
            QueueStorage::save_queue(&list);
        }

        self.engine
            .start_task(&self.rt_handle, item, self.queue.clone());
    }

    fn enqueue_batch_or_wildcard(&mut self, input: &str) {
        let re = Regex::new(r"\[(\d+)-(\d+)\]").unwrap();
        let lines: Vec<String> = input
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        for line in lines {
            if let Some(caps) = re.captures(&line) {
                let start_str = caps.get(1).map(|m| m.as_str()).unwrap_or("1");
                let end_str = caps.get(2).map(|m| m.as_str()).unwrap_or("1");
                if let (Ok(start), Ok(end)) = (start_str.parse::<u32>(), end_str.parse::<u32>()) {
                    let width = start_str.len();
                    let full_match = caps.get(0).map(|m| m.as_str()).unwrap_or("");
                    for i in start..=end.min(start + 100) {
                        let num = format!("{:0width$}", i, width = width);
                        let expanded = line.replace(full_match, &num);
                        self.enqueue_url(&expanded, None, None);
                    }
                    continue;
                }
            }
            self.enqueue_url(&line, None, None);
        }
    }

    fn poll_clipboard(&mut self) {
        if !self.clipboard_sniff_enabled || self.last_clip_check.elapsed() < Duration::from_millis(1200) {
            return;
        }
        self.last_clip_check = Instant::now();

        if let Ok(mut cb) = arboard::Clipboard::new() {
            if let Ok(text) = cb.get_text() {
                let trimmed = text.trim();
                if !trimmed.is_empty() && trimmed != self.last_clipboard_url {
                    if let Ok(parsed) = Url::parse(trimmed) {
                        let lower_path = parsed.path().to_ascii_lowercase();
                        if SNIFF_EXTENSIONS.iter().any(|ext| lower_path.ends_with(ext)) {
                            self.last_clipboard_url = trimmed.to_string();
                            if self.url_input.trim().is_empty() {
                                self.url_input = trimmed.to_string();
                                self.footer_status =
                                    format!("📋 Auto-sniffed link from clipboard: {}", trimmed);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn start_browser_hook_server(
    rt: &tokio::runtime::Handle,
    engine: DownloadEngine,
    queue: Arc<Mutex<Vec<DownloadItem>>>,
) {
    let handle = rt.clone();
    rt.spawn(async move {
        let Ok(listener) = TcpListener::bind("127.0.0.1:45780").await else {
            return;
        };

        while let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            if let Ok(n) = stream.read(&mut buf).await {
                let req_str = String::from_utf8_lossy(&buf[..n]);
                if let Some(first_line) = req_str.lines().next() {
                    let parts: Vec<&str> = first_line.split_whitespace().collect();
                    if parts.len() >= 2 && parts[1].starts_with("/add") {
                        let fake_full = format!("http://127.0.0.1:45780{}", parts[1]);
                        if let Ok(parsed) = Url::parse(&fake_full) {
                            let mut target_url = None;
                            let mut referer = None;
                            let mut cookie = None;
                            for (k, v) in parsed.query_pairs() {
                                match k.as_ref() {
                                    "url" => target_url = Some(v.into_owned()),
                                    "referer" => referer = Some(v.into_owned()),
                                    "cookie" => cookie = Some(v.into_owned()),
                                    _ => {}
                                }
                            }
                            if let Some(u) = target_url {
                                if let Ok(valid_url) = Url::parse(&u) {
                                    let fname = valid_url
                                        .path_segments()
                                        .and_then(|s| s.last())
                                        .unwrap_or("download.bin")
                                        .to_string();
                                    let now_ms = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .map(|d| d.as_millis() as u64)
                                        .unwrap_or(1);
                                    let item = DownloadItem {
                                        id: now_ms + rand::thread_rng().gen_range(1..999),
                                        url: valid_url.to_string(),
                                        file_name: fname.clone(),
                                        total_bytes: -1,
                                        downloaded_bytes: 0,
                                        status: "DOWNLOADING".to_string(),
                                        speed_bytes_per_sec: 0,
                                        thread_count: 16,
                                        category: detect_category(&fname),
                                        supports_ranges: true,
                                        referer,
                                        cookie,
                                        saved_file_path: None,
                                        sha256_hash: None,
                                        error_message: None,
                                        segment_map: String::new(),
                                    };
                                    if let Ok(mut list) = queue.lock() {
                                        list.insert(0, item.clone());
                                        QueueStorage::save_queue(&list);
                                    }
                                    engine.start_task(&handle, item, queue.clone());
                                }
                            }
                        }
                    }
                }
                let resp = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 2\r\n\r\nOK";
                let _ = stream.write_all(resp.as_bytes()).await;
            }
        }
    });
}

impl eframe::App for ShunnoGetApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(250));
        self.poll_clipboard();

        let apple_blue = Color32::from_rgb(10, 132, 255);
        let apple_deep_blue = Color32::from_rgb(0, 64, 221);
        let apple_green = Color32::from_rgb(48, 209, 88);
        let apple_orange = Color32::from_rgb(255, 159, 10);
        let apple_red = Color32::from_rgb(255, 59, 48);
        let text_primary = Color32::from_rgb(28, 28, 30);
        let text_secondary = Color32::from_rgb(142, 142, 147);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(242, 242, 247))
                    .inner_margin(egui::Margin::symmetric(22.0, 18.0)),
            )
            .show(ctx, |ui| {
                // 1. Apple Cupertino Header with Sculpted "0↓" Squircle Emblem
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, Rounding::same(13.0), apple_blue);
                    painter.rect_stroke(
                        rect.shrink(1.0),
                        Rounding::same(12.0),
                        Stroke::new(1.5, apple_deep_blue),
                    );
                    painter.circle_stroke(
                        rect.center(),
                        14.0,
                        Stroke::new(3.2, Color32::WHITE),
                    );
                    painter.text(
                        rect.center() + Vec2::new(0.0, 1.0),
                        egui::Align2::CENTER_CENTER,
                        "↓",
                        FontId::proportional(20.0),
                        apple_green,
                    );

                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Shunno Get")
                                .size(25.0)
                                .strong()
                                .color(text_primary),
                        );
                        ui.label(
                            RichText::new("Native Rust + Tokio Engine • Up to 32 Range Threads • HLS (.m3u8) • FLAC/ALAC • SHA-256")
                                .size(12.0)
                                .color(text_secondary),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let speed_labels = ["Unlimited", "10 MB/s", "5 MB/s", "2 MB/s", "512 KB/s"];
                        egui::ComboBox::from_id_source("speed_limiter")
                            .selected_text(format!("Speed: {}", speed_labels[self.speed_limit_idx]))
                            .show_ui(ui, |ui| {
                                for (idx, label) in speed_labels.iter().enumerate() {
                                    if ui
                                        .selectable_value(&mut self.speed_limit_idx, idx, *label)
                                        .changed()
                                    {
                                        let bps = match idx {
                                            1 => 10 * 1024 * 1024,
                                            2 => 5 * 1024 * 1024,
                                            3 => 2 * 1024 * 1024,
                                            4 => 512 * 1024,
                                            _ => 0,
                                        };
                                        self.engine.speed_limit_bps.store(bps, Ordering::SeqCst);
                                    }
                                }
                            });

                        ui.add_space(10.0);
                        ui.checkbox(&mut self.clipboard_sniff_enabled, "📋 Auto-Sniff Clipboard");
                    });
                });

                ui.add_space(14.0);

                // 2. URL Input, Thread Count & Batch Controls Card
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .rounding(Rounding::same(16.0))
                    .inner_margin(egui::Margin::same(16.0))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(229, 229, 234)))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let input_res = ui.add_sized(
                                [ui.available_width() - 375.0, 34.0],
                                egui::TextEdit::singleline(&mut self.url_input).hint_text(
                                    "Paste HTTP/HTTPS link, .flac/.mp4/.m3u8 stream, or wildcard track_[01-12].flac...",
                                ),
                            );

                            egui::ComboBox::from_id_source("thread_selector")
                                .selected_text(format!("{} Threads", self.selected_threads))
                                .width(105.0)
                                .show_ui(ui, |ui| {
                                    for t in [1usize, 4, 8, 16, 32] {
                                        ui.selectable_value(
                                            &mut self.selected_threads,
                                            t,
                                            format!("{} Threads", t),
                                        );
                                    }
                                });

                            let start_btn = egui::Button::new(
                                RichText::new("⚡ Start Download")
                                    .color(Color32::WHITE)
                                    .strong(),
                            )
                            .fill(apple_blue)
                            .rounding(Rounding::same(10.0))
                            .min_size(Vec2::new(135.0, 34.0));

                            if ui.add(start_btn).clicked()
                                || (input_res.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            {
                                let raw = self.url_input.clone();
                                if !raw.trim().is_empty() {
                                    self.url_input.clear();
                                    self.enqueue_batch_or_wildcard(&raw);
                                }
                            }

                            let batch_btn = egui::Button::new(
                                RichText::new("📄 .TXT Batch").color(apple_blue).strong(),
                            )
                            .fill(Color32::from_rgb(235, 244, 255))
                            .rounding(Rounding::same(10.0))
                            .min_size(Vec2::new(105.0, 34.0));

                            if ui.add(batch_btn).clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Text List", &["txt"])
                                    .pick_file()
                                {
                                    if let Ok(content) = std::fs::read_to_string(path) {
                                        self.enqueue_batch_or_wildcard(&content);
                                    }
                                }
                            }
                        });

                        ui.add_space(10.0);

                        // Category Filter Pills
                        ui.horizontal(|ui| {
                            let categories = [
                                "All",
                                "Videos",
                                "Lossless Audio",
                                "Archives",
                                "Programs",
                                "Documents",
                            ];
                            for cat in categories {
                                let is_sel = self.selected_category == cat;
                                let pill = egui::Button::new(
                                    RichText::new(cat)
                                        .size(12.5)
                                        .color(if is_sel { Color32::WHITE } else { text_primary }),
                                )
                                .fill(if is_sel {
                                    apple_blue
                                } else {
                                    Color32::from_rgb(242, 242, 247)
                                })
                                .rounding(Rounding::same(14.0));

                                if ui.add(pill).clicked() {
                                    self.selected_category = cat.to_string();
                                }
                            }

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .link(
                                            RichText::new("📂 Open Downloads Folder")
                                                .color(apple_blue)
                                                .strong(),
                                        )
                                        .clicked()
                                    {
                                        let dir = QueueStorage::target_folder("");
                                        let _ = Command::new("explorer.exe").arg(dir).spawn();
                                    }
                                },
                            );
                        });
                    });

                ui.add_space(12.0);

                // 3. Scrollable Download Queue
                let items_snapshot: Vec<DownloadItem> = if let Ok(list) = self.queue.lock() {
                    list.iter()
                        .filter(|i| {
                            self.selected_category == "All" || i.category == self.selected_category
                        })
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                };

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(ui.available_height() - 28.0)
                    .show(ui, |ui| {
                        if items_snapshot.is_empty() {
                            ui.add_space(90.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(
                                        "No downloads in this category yet.\nPaste a URL above, copy a link to your clipboard, or right-click in your browser.",
                                    )
                                    .size(14.0)
                                    .color(text_secondary),
                                );
                            });
                        } else {
                            for item in items_snapshot {
                                egui::Frame::none()
                                    .fill(Color32::WHITE)
                                    .rounding(Rounding::same(14.0))
                                    .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(229, 229, 234)))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(&item.file_name)
                                                        .size(15.0)
                                                        .strong()
                                                        .color(text_primary),
                                                );
                                                let status_color = match item.status.as_str() {
                                                    "COMPLETED" => apple_green,
                                                    "FAILED" => apple_red,
                                                    "PAUSED" => apple_orange,
                                                    _ => apple_blue,
                                                };
                                                ui.label(
                                                    RichText::new(item.badge_text())
                                                        .size(12.0)
                                                        .color(status_color),
                                                );
                                            });

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(item.size_progress_text())
                                                            .size(12.5)
                                                            .strong()
                                                            .color(text_secondary),
                                                    );
                                                },
                                            );
                                        });

                                        ui.add_space(6.0);

                                        // Cupertino Rounded Progress Bar
                                        let bar_color = if item.status == "COMPLETED" {
                                            apple_green
                                        } else {
                                            apple_blue
                                        };
                                        let (bar_rect, _) = ui.allocate_exact_size(
                                            Vec2::new(ui.available_width(), 8.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().rect_filled(
                                            bar_rect,
                                            Rounding::same(4.0),
                                            Color32::from_rgb(229, 229, 234),
                                        );
                                        let fill_w =
                                            bar_rect.width() * item.progress_fraction().clamp(0.0, 1.0);
                                        if fill_w > 1.0 {
                                            let fill_rect = Rect::from_min_size(
                                                bar_rect.min,
                                                Vec2::new(fill_w, bar_rect.height()),
                                            );
                                            ui.painter().rect_filled(
                                                fill_rect,
                                                Rounding::same(4.0),
                                                bar_color,
                                            );
                                        }

                                        ui.add_space(6.0);

                                        // Telemetry & Action Buttons
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(item.telemetry_text())
                                                    .size(11.5)
                                                    .color(text_secondary),
                                            );

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui
                                                        .add(
                                                            egui::Button::new(
                                                                RichText::new("✕")
                                                                    .color(apple_red)
                                                                    .strong(),
                                                            )
                                                            .fill(Color32::from_rgb(255, 235, 234)),
                                                        )
                                                        .clicked()
                                                    {
                                                        self.engine.pause_task(item.id);
                                                        if let Ok(mut list) = self.queue.lock() {
                                                            list.retain(|i| i.id != item.id);
                                                            QueueStorage::save_queue(&list);
                                                        }
                                                    }

                                                    if item.status == "COMPLETED" {
                                                        if ui
                                                            .add(
                                                                egui::Button::new(
                                                                    RichText::new("Show File")
                                                                        .color(apple_green)
                                                                        .strong(),
                                                                )
                                                                .fill(Color32::from_rgb(
                                                                    232, 248, 238,
                                                                )),
                                                            )
                                                            .clicked()
                                                        {
                                                            if let Some(ref path) =
                                                                item.saved_file_path
                                                            {
                                                                let _ = Command::new("explorer.exe")
                                                                    .arg(format!(
                                                                        "/select,{}",
                                                                        path
                                                                    ))
                                                                    .spawn();
                                                            }
                                                        }
                                                    } else {
                                                        if ui
                                                            .add(
                                                                egui::Button::new(
                                                                    RichText::new("Fix Link")
                                                                        .color(text_primary),
                                                                )
                                                                .fill(Color32::from_rgb(
                                                                    242, 242, 247,
                                                                )),
                                                            )
                                                            .clicked()
                                                        {
                                                            self.fix_link_target_id = Some(item.id);
                                                            self.fix_link_input = item.url.clone();
                                                        }

                                                        if item.status == "DOWNLOADING" {
                                                            if ui
                                                                .add(
                                                                    egui::Button::new(
                                                                        RichText::new("Pause")
                                                                            .color(apple_orange)
                                                                            .strong(),
                                                                    )
                                                                    .fill(Color32::from_rgb(
                                                                        255, 245, 229,
                                                                    )),
                                                                )
                                                                .clicked()
                                                            {
                                                                self.engine.pause_task(item.id);
                                                            }
                                                        } else if ui
                                                            .add(
                                                                egui::Button::new(
                                                                    RichText::new("Resume")
                                                                        .color(apple_blue)
                                                                        .strong(),
                                                                )
                                                                .fill(Color32::from_rgb(
                                                                    235, 244, 255,
                                                                )),
                                                            )
                                                            .clicked()
                                                        {
                                                            self.engine.start_task(
                                                                &self.rt_handle,
                                                                item.clone(),
                                                                self.queue.clone(),
                                                            );
                                                        }
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(8.0);
                            }
                        }
                    });

                // 4. Footer Status Bar
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(
                        RichText::new(&self.footer_status)
                            .size(11.5)
                            .color(text_secondary),
                    );
                });
            });

        // Modal Dialog: Refresh Expired Download Link (Fix Link)
        if let Some(target_id) = self.fix_link_target_id {
            let mut open = true;
            egui::Window::new("Refresh Expired Download Link")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(
                        "Paste a fresh CDN URL or mirror to resume from the exact byte offset without losing downloaded data:",
                    );
                    ui.add_space(6.0);
                    ui.add_sized(
                        [440.0, 28.0],
                        egui::TextEdit::singleline(&mut self.fix_link_input),
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Update Link & Resume")
                                        .color(Color32::WHITE)
                                        .strong(),
                                )
                                .fill(apple_blue),
                            )
                            .clicked()
                        {
                            let new_url = self.fix_link_input.trim().to_string();
                            if new_url.starts_with("http") {
                                let mut updated_item = None;
                                if let Ok(mut list) = self.queue.lock() {
                                    if let Some(it) = list.iter_mut().find(|i| i.id == target_id) {
                                        it.url = new_url;
                                        updated_item = Some(it.clone());
                                    }
                                    QueueStorage::save_queue(&list);
                                }
                                if let Some(it) = updated_item {
                                    self.engine.start_task(
                                        &self.rt_handle,
                                        it,
                                        self.queue.clone(),
                                    );
                                }
                            }
                            self.fix_link_target_id = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.fix_link_target_id = None;
                        }
                    });
                });
            if !open {
                self.fix_link_target_id = None;
            }
        }
    }
}
