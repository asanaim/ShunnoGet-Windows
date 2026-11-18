#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;
mod state;
mod ui_chrome;

use downloader::ShunnoGetDesktop;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::{Rect, WebView, WebViewBuilder};

use state::{
    extract_host, sanitize_url, BookmarkEntry, BrowserStorage, GlobalSettings, HistoryEntry,
    SavedTab, SitePolicy,
};

const CHROME_BAR_HEIGHT: f64 = 86.0;

enum AppEvent {
    ChromeIpc(String),
    ContentIpc(usize, String),
    DownloadIntercepted(String),
    NewWindowBlockedOrOpened(String),
    RefreshChrome,
}

struct BrowserTab {
    id: usize,
    title: String,
    url: String,
    incognito: bool,
    webview: WebView,
}

#[derive(Serialize)]
struct TabSummary {
    id: usize,
    title: String,
    url: String,
    incognito: bool,
}

#[derive(Serialize)]
struct ChromeSnapshot {
    tabs: Vec<TabSummary>,
    active_tab: usize,
    active_url: String,
    active_host: String,
    site: SitePolicy,
    global: GlobalSettings,
    blocked_count: usize,
    streams: Vec<String>,
    bookmarks: Vec<BookmarkEntry>,
    history: Vec<HistoryEntry>,
    downloads: Vec<downloader::DownloadTask>,
}

fn build_content_init_script(policy: &SitePolicy, global: &GlobalSettings, user_js: &str) -> String {
    let mut script = String::new();

    // 1. Brave-Style Background YouTube & Media Playback (Page Visibility API lock)
    if global.background_youtube_play {
        script.push_str(
            r#"
            (function() {
                try {
                    Object.defineProperty(document, 'hidden', { get: () => false, configurable: true });
                    Object.defineProperty(document, 'visibilityState', { get: () => 'visible', configurable: true });
                    window.addEventListener('visibilitychange', e => e.stopImmediatePropagation(), true);
                    document.addEventListener('visibilitychange', e => e.stopImmediatePropagation(), true);
                } catch(e) {}
            })();
            "#,
        );
    }

    // 2. Network / Fetch / XHR Media & Lossless Audio Stream Sniffer + Ad/Tracker Blocker
    let adblock_flag = if policy.adblock { "true" } else { "false" };
    script.push_str(&format!(
        r#"
        (function() {{
            const ADBLOCK_ON = {adblock_flag};
            const AD_HOSTS = [
                'doubleclick.net', 'googlesyndication.com', 'googleadservices.com',
                'adnxs.com', 'adsrvr.org', 'taboola.com', 'outbrain.com',
                'popads.net', 'popcash.net', 'propellerads.com', 'exoclick.com',
                'scorecardresearch.com', 'criteo.com', 'pubmatic.com', 'rubiconproject.com'
            ];
            const MEDIA_EXTS = ['.mp4', '.m3u8', '.flac', '.m4a', '.wav', '.mp3', '.zip', '.iso', '.exe', '.msi', '.apk'];

            function inspectUrl(raw) {{
                if (!raw) return false;
                const str = String(raw);
                const lower = str.split('?')[0].toLowerCase();
                if (ADBLOCK_ON && AD_HOSTS.some(h => lower.includes(h))) {{
                    if (window.ipc) window.ipc.postMessage('AD_BLOCKED');
                    return true;
                }}
                if (MEDIA_EXTS.some(ext => lower.endsWith(ext)) && str.startsWith('http')) {{
                    if (window.ipc) window.ipc.postMessage('STREAM:' + str);
                }}
                return false;
            }}

            const origFetch = window.fetch;
            window.fetch = function(input, init) {{
                const u = (typeof input === 'string') ? input : (input && input.url);
                if (inspectUrl(u)) return Promise.reject(new Error('Blocked by Shunno AdBlock'));
                return origFetch.apply(this, arguments);
            }};

            const origOpen = XMLHttpRequest.prototype.open;
            XMLHttpRequest.prototype.open = function(method, url) {{
                if (inspectUrl(url)) {{
                    this._shunnoBlocked = true;
                    return;
                }}
                return origOpen.apply(this, arguments);
            }};
            const origSend = XMLHttpRequest.prototype.send;
            XMLHttpRequest.prototype.send = function() {{
                if (this._shunnoBlocked) return;
                return origSend.apply(this, arguments);
            }};

            window.addEventListener('DOMContentLoaded', () => {{
                if (window.ipc) window.ipc.postMessage('META:' + JSON.stringify({{ title: document.title, url: location.href }}));
                document.querySelectorAll('video source, audio source, video[src], audio[src]').forEach(el => {{
                    if (el.src) inspectUrl(el.src);
                }});
            }});
        }})();
        "#
    ));

    // 3. Per-Site NBPro Controls: JavaScript Gate, Image Blocker, Desktop User-Agent, OLED Force-Dark, SponsorBlock & Cookie Rejector
    if !policy.js_enabled {
        script.push_str(
            r#"
            (function() {
                window.addEventListener('DOMContentLoaded', () => {
                    document.querySelectorAll('script').forEach(s => s.remove());
                });
            })();
            "#,
        );
    }

    if !policy.images_enabled {
        script.push_str(
            r#"
            (function() {
                const st = document.createElement('style');
                st.textContent = 'img, picture, svg, video[poster], [style*="background-image"] { display: none !important; background-image: none !important; }';
                document.documentElement.appendChild(st);
            })();
            "#,
        );
    }

    if policy.desktop_ua {
        script.push_str(
            r#"
            (function() {
                try {
                    Object.defineProperty(navigator, 'userAgent', {
                        get: () => 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36'
                    });
                } catch(e) {}
            })();
            "#,
        );
    }

    if policy.oled_dark {
        script.push_str(
            r#"
            (function() {
                const st = document.createElement('style');
                st.textContent = 'html, body, div, section, article, main, header, footer, nav, aside { background-color: #000000 !important; color: #E5E5EA !important; border-color: #2C2C2E !important; } a { color: #0A84FF !important; }';
                document.documentElement.appendChild(st);
            })();
            "#,
        );
    }

    if policy.adblock || policy.sponsorblock || policy.auto_reject_cookies {
        let hide_ads = if policy.adblock || policy.sponsorblock {
            "ytd-reel-shelf-renderer, ytd-rich-section-renderer, .ad-showing, .ytp-ad-module, ins.adsbygoogle, [id^=\"google_ads_\"], [class*=\"taboola\"], [class*=\"outbrain\"] { display: none !important; }"
        } else {
            ""
        };
        let auto_skip = if policy.sponsorblock { "true" } else { "false" };
        let reject_cookies = if policy.auto_reject_cookies { "true" } else { "false" };

        script.push_str(&format!(
            r#"
            (function() {{
                const st = document.createElement('style');
                st.textContent = `{hide_ads}`;
                document.documentElement.appendChild(st);

                setInterval(() => {{
                    if ({auto_skip}) {{
                        const sk = document.querySelector('.ytp-ad-skip-button, .ytp-ad-skip-button-modern');
                        if (sk) sk.click();
                        const v = document.querySelector('video');
                        if (v && document.querySelector('.ad-showing')) {{
                            v.playbackRate = 16.0;
                            v.muted = true;
                        }}
                    }}
                    if ({reject_cookies}) {{
                        document.querySelectorAll('[id*="reject"], [class*="reject"], #onetrust-reject-all-handler, .fc-cta-do-not-consent').forEach(b => b.click());
                    }}
                }}, 900);
            }})();
            "#
        ));
    }

    // 4. Custom User-Scripts (Downloads/ShunnoBrowser/scripts/*.js)
    if !user_js.is_empty() {
        script.push_str(user_js);
    }

    script
}

fn create_tab_webview(
    window: &Window,
    tab_id: usize,
    url: &str,
    policy: &SitePolicy,
    global: &GlobalSettings,
    proxy: EventLoopProxy<AppEvent>,
    drawer_open: bool,
) -> wry::Result<WebView> {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let content_h = (size.height - CHROME_BAR_HEIGHT).max(100.0);
    let host = extract_host(url);
    let user_js = BrowserStorage::load_matching_user_scripts(&host);
    let init_script = build_content_init_script(policy, global, &user_js);

    let ipc_proxy = proxy.clone();
    let dl_proxy = proxy.clone();
    let win_proxy = proxy.clone();

    let wv = WebViewBuilder::new_as_child(window)
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, CHROME_BAR_HEIGHT).into(),
            size: LogicalSize::new(size.width.max(400.0), content_h).into(),
        })
        .with_url(url)
        .with_initialization_script(&init_script)
        .with_ipc_handler(move |req| {
            let _ = ipc_proxy.send_event(AppEvent::ContentIpc(tab_id, req.body().clone()));
        })
        .with_download_started_handler(move |dl_url, _dest| {
            let _ = dl_proxy.send_event(AppEvent::DownloadIntercepted(dl_url));
            false // Route to Shunno Get 32-Thread Engine
        })
        .with_new_window_req_handler(move |popup_url| {
            let _ = win_proxy.send_event(AppEvent::NewWindowBlockedOrOpened(popup_url));
            false
        })
        .build()?;

    let _ = wv.set_visible(!drawer_open);
    Ok(wv)
}

fn layout_webviews(window: &Window, chrome_wv: &WebView, tabs: &[BrowserTab], active_idx: usize, drawer_open: bool) {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let w = size.width.max(400.0);
    let h = size.height.max(300.0);

    let chrome_h = if drawer_open { h } else { CHROME_BAR_HEIGHT };
    let _ = chrome_wv.set_bounds(Rect {
        position: LogicalPosition::new(0.0, 0.0).into(),
        size: LogicalSize::new(w, chrome_h).into(),
    });

    let content_h = (h - CHROME_BAR_HEIGHT).max(100.0);
    for (idx, tab) in tabs.iter().enumerate() {
        let _ = tab.webview.set_bounds(Rect {
            position: LogicalPosition::new(0.0, CHROME_BAR_HEIGHT).into(),
            size: LogicalSize::new(w, content_h).into(),
        });
        let _ = tab.webview.set_visible(!drawer_open && idx == active_idx);
    }
}

fn main() -> wry::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to start Tokio runtime");
    let rt_handle = rt.handle().clone();

    let mut global_settings = BrowserStorage::load_global_settings();
    let mut site_policies = BrowserStorage::load_site_policies();
    let mut bookmarks = BrowserStorage::load_bookmarks();
    let mut history = BrowserStorage::load_history();
    let engine = Arc::new(ShunnoGetDesktop::new(global_settings.speed_limit_bps));

    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Shunno Browser — NBPro + Brave Background Audio + 32-Thread Shunno Get")
        .with_inner_size(LogicalSize::new(1220.0, 780.0))
        .build(&event_loop)
        .unwrap();

    let chrome_proxy = proxy.clone();
    let chrome_wv = WebViewBuilder::new_as_child(&window)
        .with_bounds(Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(1220.0, CHROME_BAR_HEIGHT).into(),
        })
        .with_html(ui_chrome::CHROME_HTML)
        .with_ipc_handler(move |req| {
            let _ = chrome_proxy.send_event(AppEvent::ChromeIpc(req.body().clone()));
        })
        .build()?;

    let next_tab_id = Arc::new(AtomicUsize::new(1));
    let mut tabs: Vec<BrowserTab> = Vec::new();
    let mut active_tab_idx: usize = 0;
    let mut drawer_open = false;
    let mut blocked_count: usize = 0;
    let mut sniffed_streams: Vec<String> = Vec::new();

    // Restore previous session or open default start page
    let saved_session = BrowserStorage::load_session();
    let initial_urls: Vec<String> = if saved_session.is_empty() {
        vec!["https://duckduckgo.com".to_string()]
    } else {
        saved_session.into_iter().map(|t| t.url).collect()
    };

    for u in initial_urls {
        let host = extract_host(&u);
        let pol = site_policies.get(&host).cloned().unwrap_or_default();
        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
        if let Ok(wv) = create_tab_webview(&window, tid, &u, &pol, &global_settings, proxy.clone(), false) {
            tabs.push(BrowserTab {
                id: tid,
                title: u.clone(),
                url: u,
                incognito: false,
                webview: wv,
            });
        }
    }

    // Periodic background refresh timer to keep download progress bars & speeds updated smoothly
    let ticker_proxy = proxy.clone();
    rt_handle.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(600)).await;
            let _ = ticker_proxy.send_event(AppEvent::RefreshChrome);
        }
    });

    let push_chrome_state = |chrome_wv: &WebView,
                             tabs: &[BrowserTab],
                             active_idx: usize,
                             site_policies: &HashMap<String, SitePolicy>,
                             global_settings: &GlobalSettings,
                             blocked_count: usize,
                             sniffed_streams: &[String],
                             bookmarks: &[BookmarkEntry],
                             history: &[HistoryEntry],
                             engine: &ShunnoGetDesktop| {
        let active_url = tabs.get(active_idx).map(|t| t.url.clone()).unwrap_or_default();
        let active_host = extract_host(&active_url);
        let site = site_policies.get(&active_host).cloned().unwrap_or_default();
        let downloads = engine.tasks.lock().map(|l| l.clone()).unwrap_or_default();

        let snap = ChromeSnapshot {
            tabs: tabs
                .iter()
                .map(|t| TabSummary {
                    id: t.id,
                    title: t.title.clone(),
                    url: t.url.clone(),
                    incognito: t.incognito,
                })
                .collect(),
            active_tab: active_idx,
            active_url,
            active_host,
            site,
            global: global_settings.clone(),
            blocked_count,
            streams: sniffed_streams.to_vec(),
            bookmarks: bookmarks.to_vec(),
            history: history.to_vec(),
            downloads,
        };

        if let Ok(json) = serde_json::to_string(&snap) {
            let js = format!("if (window.renderChromeState) window.renderChromeState({});", json);
            let _ = chrome_wv.evaluate_script(&js);
        }
    };

    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
            }

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                let to_save: Vec<SavedTab> = tabs
                    .iter()
                    .filter(|t| !t.incognito)
                    .map(|t| SavedTab {
                        title: t.title.clone(),
                        url: t.url.clone(),
                    })
                    .collect();
                BrowserStorage::save_session(&to_save);
                *control_flow = ControlFlow::Exit;
            }

            Event::UserEvent(AppEvent::RefreshChrome) => {
                push_chrome_state(
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::DownloadIntercepted(dl_url)) => {
                let ref_url = tabs.get(active_tab_idx).map(|t| t.url.clone());
                engine.enqueue(&rt_handle, dl_url, global_settings.default_threads, ref_url, None);
                drawer_open = true;
                layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                let _ = chrome_wv.evaluate_script("toggleDrawer('downloads');");
                push_chrome_state(
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::NewWindowBlockedOrOpened(popup_url)) => {
                let lower = popup_url.to_ascii_lowercase();
                let is_ad_popup = lower.contains("popads")
                    || lower.contains("popcash")
                    || lower.contains("doubleclick")
                    || lower.contains("propellerads");
                if global_settings.block_popups && is_ad_popup {
                    blocked_count += 1;
                } else if popup_url.starts_with("http") {
                    let clean = sanitize_url(&popup_url, global_settings.strip_tracking_params);
                    let host = extract_host(&clean);
                    let pol = site_policies.get(&host).cloned().unwrap_or_default();
                    let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                    if let Ok(wv) = create_tab_webview(&window, tid, &clean, &pol, &global_settings, proxy.clone(), drawer_open) {
                        tabs.push(BrowserTab {
                            id: tid,
                            title: clean.clone(),
                            url: clean,
                            incognito: false,
                            webview: wv,
                        });
                        active_tab_idx = tabs.len() - 1;
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                    }
                }
                push_chrome_state(
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::ContentIpc(tab_id, msg)) => {
                if msg == "AD_BLOCKED" {
                    blocked_count += 1;
                } else if let Some(stream_url) = msg.strip_prefix("STREAM:") {
                    if !sniffed_streams.iter().any(|s| s == stream_url) {
                        sniffed_streams.insert(0, stream_url.to_string());
                        if sniffed_streams.len() > 25 {
                            sniffed_streams.truncate(25);
                        }
                    }
                } else if let Some(meta_json) = msg.strip_prefix("META:") {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(meta_json) {
                        let title = val.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let url = val.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if let Some(tab) = tabs.iter_mut().find(|t| t.id == tab_id) {
                            if !title.is_empty() {
                                tab.title = title.clone();
                            }
                            if !url.is_empty() {
                                tab.url = url.clone();
                                if !tab.incognito {
                                    let now = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .map(|d| d.as_secs())
                                        .unwrap_or(0);
                                    history.insert(
                                        0,
                                        HistoryEntry {
                                            title: if title.is_empty() { url.clone() } else { title },
                                            url,
                                            visited_at: now,
                                        },
                                    );
                                    BrowserStorage::save_history(&history);
                                }
                            }
                        }
                    }
                }
                push_chrome_state(
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &engine,
                );
            }

            Event::UserEvent(AppEvent::ChromeIpc(cmd)) => {
                if let Some(open_flag) = cmd.strip_prefix("DRAWER_OPEN:") {
                    drawer_open = open_flag == "1";
                    layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                } else if cmd == "BACK" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("history.back();");
                    }
                } else if cmd == "FORWARD" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("history.forward();");
                    }
                } else if cmd == "RELOAD" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let _ = tab.webview.evaluate_script("location.reload();");
                    }
                } else if let Some(raw_nav) = cmd.strip_prefix("NAV:") {
                    let clean = sanitize_url(raw_nav, global_settings.strip_tracking_params);
                    sniffed_streams.clear();
                    if let Some(tab) = tabs.get_mut(active_tab_idx) {
                        tab.url = clean.clone();
                        let _ = tab.webview.load_url(&clean);
                    }
                } else if cmd == "NEW_TAB" || cmd == "NEW_INCOGNITO" {
                    let is_incog = cmd == "NEW_INCOGNITO";
                    let start = "https://duckduckgo.com".to_string();
                    let pol = SitePolicy::default();
                    let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                    if let Ok(wv) = create_tab_webview(&window, tid, &start, &pol, &global_settings, proxy.clone(), drawer_open) {
                        tabs.push(BrowserTab {
                            id: tid,
                            title: if is_incog { "Private Tab".to_string() } else { "New Tab".to_string() },
                            url: start,
                            incognito: is_incog,
                            webview: wv,
                        });
                        active_tab_idx = tabs.len() - 1;
                        layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("SWITCH_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < tabs.len() {
                            active_tab_idx = idx;
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("CLOSE_TAB:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if tabs.len() > 1 && idx < tabs.len() {
                            tabs.remove(idx);
                            if active_tab_idx >= tabs.len() {
                                active_tab_idx = tabs.len() - 1;
                            }
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                        }
                    }
                } else if cmd == "TOGGLE_BG_AUDIO" {
                    global_settings.background_youtube_play = !global_settings.background_youtube_play;
                    BrowserStorage::save_global_settings(&global_settings);
                } else if let Some(gkey) = cmd.strip_prefix("TOGGLE_GLOBAL:") {
                    match gkey {
                        "bg_audio" => global_settings.background_youtube_play = !global_settings.background_youtube_play,
                        "strip_tracking" => global_settings.strip_tracking_params = !global_settings.strip_tracking_params,
                        "block_popups" => global_settings.block_popups = !global_settings.block_popups,
                        _ => {}
                    }
                    BrowserStorage::save_global_settings(&global_settings);
                } else if let Some(skey) = cmd.strip_prefix("TOGGLE_SITE:") {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let host = extract_host(&tab.url);
                        let pol = site_policies.entry(host.clone()).or_default();
                        match skey {
                            "adblock" => pol.adblock = !pol.adblock,
                            "js_enabled" => pol.js_enabled = !pol.js_enabled,
                            "images_enabled" => pol.images_enabled = !pol.images_enabled,
                            "desktop_ua" => pol.desktop_ua = !pol.desktop_ua,
                            "oled_dark" => pol.oled_dark = !pol.oled_dark,
                            "sponsorblock" => pol.sponsorblock = !pol.sponsorblock,
                            "auto_reject_cookies" => pol.auto_reject_cookies = !pol.auto_reject_cookies,
                            _ => {}
                        }
                        let updated_pol = pol.clone();
                        BrowserStorage::save_site_policies(&site_policies);

                        // Recreate tab WebView with updated site policy & reload
                        let current_url = tab.url.clone();
                        let is_incog = tab.incognito;
                        let tid = next_tab_id.fetch_add(1, Ordering::SeqCst);
                        if let Ok(new_wv) = create_tab_webview(&window, tid, &current_url, &updated_pol, &global_settings, proxy.clone(), drawer_open) {
                            tabs[active_tab_idx] = BrowserTab {
                                id: tid,
                                title: tab.title.clone(),
                                url: current_url,
                                incognito: is_incog,
                                webview: new_wv,
                            };
                            layout_webviews(&window, &chrome_wv, &tabs, active_tab_idx, drawer_open);
                        }
                    }
                } else if let Some(spd) = cmd.strip_prefix("SET_PLAYBACK_SPEED:") {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let js = format!("document.querySelectorAll('video,audio').forEach(m => m.playbackRate = {});", spd);
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "READER_MODE" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let js = r#"
                            (function() {
                                const text = (document.querySelector('article') || document.body).innerText;
                                document.body.innerHTML = '<div style="max-width:720px;margin:36px auto;padding:20px;font:19px/1.7 -apple-system,BlinkMacSystemFont,Segoe UI,sans-serif;color:#1C1C1E;background:#F2F2F7;">' +
                                    text.split('\n').filter(p => p.trim().length > 0).map(p => '<p style="margin-bottom:16px;">' + p + '</p>').join('') + '</div>';
                            })();
                        "#;
                        let _ = tab.webview.evaluate_script(js);
                    }
                } else if let Some(query) = cmd.strip_prefix("FIND:") {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let escaped = query.replace('\\', "\\\\").replace('\'', "\\'");
                        let js = format!("if ('{}') window.find('{}', false, false, true);", escaped, escaped);
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "FIND_NEXT" || cmd == "FIND_PREV" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let backwards = if cmd == "FIND_PREV" { "true" } else { "false" };
                        let js = format!("window.find('', false, {}, true);", backwards);
                        let _ = tab.webview.evaluate_script(&js);
                    }
                } else if cmd == "ADD_BOOKMARK" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        if !bookmarks.iter().any(|b| b.url == tab.url) {
                            let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                            bookmarks.insert(0, BookmarkEntry {
                                title: tab.title.clone(),
                                url: tab.url.clone(),
                                created_at: now,
                            });
                            BrowserStorage::save_bookmarks(&bookmarks);
                        }
                    }
                } else if let Some(idx_str) = cmd.strip_prefix("DEL_BOOKMARK:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < bookmarks.len() {
                            bookmarks.remove(idx);
                            BrowserStorage::save_bookmarks(&bookmarks);
                        }
                    }
                } else if cmd == "EXPORT_TXT_BOOKMARKS" {
                    let _ = BrowserStorage::export_portable_txt_bookmarks(&bookmarks);
                } else if cmd == "IMPORT_TXT_BOOKMARKS" {
                    let _ = BrowserStorage::import_portable_txt_bookmarks(&mut bookmarks);
                } else if cmd == "CLEAR_HISTORY" {
                    history.clear();
                    BrowserStorage::save_history(&history);
                } else if cmd == "SAVE_OFFLINE_HTML" {
                    if let Some(tab) = tabs.get(active_tab_idx) {
                        let safe_name = tab
                            .title
                            .chars()
                            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
                            .take(40)
                            .collect::<String>()
                            .trim()
                            .replace(' ', "_");
                        let fname = if safe_name.is_empty() { "snapshot.html".to_string() } else { format!("{}.html", safe_name) };
                        let out = BrowserStorage::downloads_root_dir().join(&fname);
                        let _ = fs::write(
                            out,
                            format!("<!-- Saved Offline by Shunno Browser -->\n<meta http-equiv=\"refresh\" content=\"0;url={}\">", tab.url),
                        );
                    }
                } else if cmd == "OPEN_SCRIPTS_FOLDER" {
                    let dir = BrowserStorage::user_scripts_dir();
                    let _ = Command::new("explorer.exe").arg(dir).spawn();
                } else if cmd == "OPEN_DOWNLOADS_FOLDER" {
                    let dir = BrowserStorage::downloads_root_dir();
                    let _ = Command::new("explorer.exe").arg(dir).spawn();
                } else if let Some(limit_str) = cmd.strip_prefix("SET_SPEED_LIMIT:") {
                    if let Ok(bps) = limit_str.parse::<u64>() {
                        global_settings.speed_limit_bps = bps;
                        engine.speed_limit_bps.store(bps, Ordering::SeqCst);
                        BrowserStorage::save_global_settings(&global_settings);
                    }
                } else if let Some(payload) = cmd.strip_prefix("ENQUEUE_DL:") {
                    if let Some((threads_str, raw_urls)) = payload.split_once('|') {
                        let threads = threads_str.parse::<usize>().unwrap_or(16);
                        let ref_url = tabs.get(active_tab_idx).map(|t| t.url.clone());
                        let re = Regex::new(r"\[(\d+)-(\d+)\]").unwrap();
                        if let Some(caps) = re.captures(raw_urls) {
                            let s_str = caps.get(1).map(|m| m.as_str()).unwrap_or("1");
                            let e_str = caps.get(2).map(|m| m.as_str()).unwrap_or("1");
                            if let (Ok(start), Ok(end)) = (s_str.parse::<u32>(), e_str.parse::<u32>()) {
                                let width = s_str.len();
                                let full_m = caps.get(0).map(|m| m.as_str()).unwrap_or("");
                                for i in start..=end.min(start + 100) {
                                    let num = format!("{:0width$}", i, width = width);
                                    let expanded = raw_urls.replace(full_m, &num);
                                    engine.enqueue(&rt_handle, expanded, threads, ref_url.clone(), None);
                                }
                            }
                        } else {
                            engine.enqueue(&rt_handle, raw_urls.to_string(), threads, ref_url, None);
                        }
                    }
                } else if let Some(id_str) = cmd.strip_prefix("DL_PAUSE:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        engine.pause(id);
                    }
                } else if let Some(id_str) = cmd.strip_prefix("DL_RESUME:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        engine.resume_or_restart(&rt_handle, id, None);
                    }
                } else if let Some(payload) = cmd.strip_prefix("DL_FIX:") {
                    if let Some((id_str, new_url)) = payload.split_once('|') {
                        if let Ok(id) = id_str.parse::<u64>() {
                            engine.resume_or_restart(&rt_handle, id, Some(new_url.to_string()));
                        }
                    }
                } else if let Some(id_str) = cmd.strip_prefix("DL_DEL:") {
                    if let Ok(id) = id_str.parse::<u64>() {
                        engine.delete(id);
                    }
                } else if let Some(path) = cmd.strip_prefix("DL_SHOW:") {
                    if !path.is_empty() {
                        let _ = Command::new("explorer.exe").arg(format!("/select,{}", path)).spawn();
                    }
                }

                push_chrome_state(
                    &chrome_wv,
                    &tabs,
                    active_tab_idx,
                    &site_policies,
                    &global_settings,
                    blocked_count,
                    &sniffed_streams,
                    &bookmarks,
                    &history,
                    &engine,
                );
            }

            _ => {}
        }
    });
}
