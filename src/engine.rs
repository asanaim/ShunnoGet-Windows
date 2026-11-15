use crate::models::{DownloadItem, QueueStorage};
use futures_util::StreamExt;
use reqwest::header::{ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, COOKIE, RANGE, REFERER, USER_AGENT};
use reqwest::{Client, StatusCode};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct DownloadEngine {
    client: Client,
    active_flags: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    pub speed_limit_bps: Arc<AtomicU64>, // 0 = Unlimited
}

impl DownloadEngine {
    pub fn new() -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .pool_max_idle_per_host(64)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            active_flags: Arc::new(Mutex::new(HashMap::new())),
            speed_limit_bps: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn pause_task(&self, id: u64) {
        if let Ok(mut map) = self.active_flags.lock() {
            if let Some(flag) = map.remove(&id) {
                flag.store(false, Ordering::SeqCst);
            }
        }
    }

    pub fn start_task(
        &self,
        rt: &tokio::runtime::Handle,
        item: DownloadItem,
        shared_queue: Arc<Mutex<Vec<DownloadItem>>>,
    ) {
        self.pause_task(item.id);
        let running = Arc::new(AtomicBool::new(true));
        if let Ok(mut map) = self.active_flags.lock() {
            map.insert(item.id, running.clone());
        }

        update_item(&shared_queue, item.id, |it| {
            it.status = "DOWNLOADING".to_string();
            it.error_message = None;
        });

        let client = self.client.clone();
        let speed_limit = self.speed_limit_bps.clone();
        let active_flags = self.active_flags.clone();

        rt.spawn(async move {
            let part_file = QueueStorage::parts_dir().join(format!("{}_{}.part", item.id, item.file_name));

            let res = if item.url.to_ascii_lowercase().contains(".m3u8") {
                download_hls_stream(&client, &item, &part_file, &running, &shared_queue).await
            } else {
                download_multithreaded(
                    &client,
                    &item,
                    &part_file,
                    &running,
                    &speed_limit,
                    &shared_queue,
                )
                .await
            };

            match res {
                Ok(()) => {}
                Err(err) => {
                    if running.load(Ordering::SeqCst) {
                        update_item(&shared_queue, item.id, |it| {
                            it.status = "FAILED".to_string();
                            it.speed_bytes_per_sec = 0;
                            it.error_message = Some(err);
                        });
                    } else {
                        update_item(&shared_queue, item.id, |it| {
                            it.status = "PAUSED".to_string();
                            it.speed_bytes_per_sec = 0;
                        });
                    }
                }
            }

            if let Ok(mut map) = active_flags.lock() {
                map.remove(&item.id);
            }
        });
    }
}

fn update_item<F: FnOnce(&mut DownloadItem)>(
    queue: &Arc<Mutex<Vec<DownloadItem>>>,
    id: u64,
    f: F,
) {
    if let Ok(mut list) = queue.lock() {
        if let Some(item) = list.iter_mut().find(|i| i.id == id) {
            f(item);
        }
        QueueStorage::save_queue(&list);
    }
}

fn apply_headers(
    mut req: reqwest::RequestBuilder,
    item: &DownloadItem,
) -> reqwest::RequestBuilder {
    req = req.header(
        USER_AGENT,
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ShunnoGet-Rust/1.0",
    );
    if let Some(ref referer) = item.referer {
        if !referer.trim().is_empty() {
            req = req.header(REFERER, referer.trim());
        }
    }
    if let Some(ref cookie) = item.cookie {
        if !cookie.trim().is_empty() {
            req = req.header(COOKIE, cookie.trim());
        }
    }
    req
}

#[cfg(windows)]
fn write_all_positioned(file: &File, mut buf: &[u8], mut offset: u64) -> std::io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        let written = file.seek_write(buf, offset)?;
        if written == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "seek_write wrote 0 bytes",
            ));
        }
        buf = &buf[written..];
        offset += written as u64;
    }
    Ok(())
}

#[cfg(not(windows))]
fn write_all_positioned(file: &File, buf: &[u8], offset: u64) -> std::io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.write_all_at(buf, offset)
}

async fn download_multithreaded(
    client: &Client,
    item: &DownloadItem,
    part_file: &Path,
    running: &Arc<AtomicBool>,
    speed_limit: &Arc<AtomicU64>,
    shared_queue: &Arc<Mutex<Vec<DownloadItem>>>,
) -> Result<(), String> {
    let mut content_length: Option<u64> = None;
    let mut accept_ranges = false;

    // 1. Probe with HEAD
    if let Ok(resp) = apply_headers(client.head(&item.url), item).send().await {
        content_length = resp
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        accept_ranges = resp
            .headers()
            .get(ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.eq_ignore_ascii_case("bytes"))
            .unwrap_or(false)
            || resp.status() == StatusCode::PARTIAL_CONTENT;
    }

    // 2. Fallback probe with Ranged GET (bytes=0-0)
    if content_length.is_none() {
        if let Ok(resp) = apply_headers(client.get(&item.url), item)
            .header(RANGE, "bytes=0-0")
            .send()
            .await
        {
            if resp.status() == StatusCode::PARTIAL_CONTENT {
                accept_ranges = true;
                content_length = resp
                    .headers()
                    .get(CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.split('/').nth(1))
                    .and_then(|s| s.parse::<u64>().ok());
            } else {
                content_length = resp
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
            }
        }
    }

    let total_len = content_length.unwrap_or(0);
    let use_multithread = accept_ranges && total_len > 512 * 1024;
    let threads = if use_multithread {
        item.thread_count.clamp(1, 32)
    } else {
        1
    };

    update_item(shared_queue, item.id, |it| {
        it.total_bytes = if total_len > 0 { total_len as i64 } else { -1 };
        it.supports_ranges = use_multithread;
    });

    if use_multithread && total_len > 0 {
        let shared_file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(part_file)
            .map_err(|e| e.to_string())?;

        if shared_file.metadata().map(|m| m.len()).unwrap_or(0) != total_len {
            shared_file.set_len(total_len).map_err(|e| e.to_string())?;
        }
        let shared_file = Arc::new(shared_file);

        let chunk_size = total_len / (threads as u64);
        let thread_progress: Arc<Vec<AtomicU64>> =
            Arc::new((0..threads).map(|_| AtomicU64::new(0)).collect());
        let mut thread_totals = vec![0u64; threads];

        for i in 0..threads {
            let start = (i as u64) * chunk_size;
            let end = if i == threads - 1 {
                total_len - 1
            } else {
                start + chunk_size - 1
            };
            let seg_total = end - start + 1;
            thread_totals[i] = seg_total;

            let offset_path = QueueStorage::parts_dir().join(format!("{}_t{}.offset", item.id, i));
            if let Ok(txt) = fs::read_to_string(&offset_path) {
                if let Ok(saved_offset) = txt.trim().parse::<u64>() {
                    let done = saved_offset.saturating_sub(start).min(seg_total);
                    thread_progress[i].store(done, Ordering::SeqCst);
                }
            }
        }

        // Spawn live telemetry & segment visualizer monitor
        let mon_running = running.clone();
        let mon_progress = thread_progress.clone();
        let mon_totals = thread_totals.clone();
        let mon_queue = shared_queue.clone();
        let task_id = item.id;

        let monitor_handle = tokio::spawn(async move {
            let mut last_bytes: u64 = mon_progress.iter().map(|a| a.load(Ordering::Relaxed)).sum();
            while mon_running.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let current_bytes: u64 =
                    mon_progress.iter().map(|a| a.load(Ordering::Relaxed)).sum();
                let speed = current_bytes.saturating_sub(last_bytes) * 2;
                last_bytes = current_bytes;

                let mut seg_bar = String::with_capacity(mon_totals.len() + 2);
                seg_bar.push('[');
                for (idx, tot) in mon_totals.iter().enumerate() {
                    let done = mon_progress[idx].load(Ordering::Relaxed);
                    let ratio = if *tot > 0 {
                        (done as f64) / (*tot as f64)
                    } else {
                        1.0
                    };
                    let ch = if ratio >= 0.99 {
                        '█'
                    } else if ratio >= 0.50 {
                        '▓'
                    } else if ratio > 0.05 {
                        '▒'
                    } else {
                        '░'
                    };
                    seg_bar.push(ch);
                }
                seg_bar.push(']');

                if let Ok(mut list) = mon_queue.lock() {
                    if let Some(it) = list.iter_mut().find(|i| i.id == task_id) {
                        it.downloaded_bytes = current_bytes;
                        it.speed_bytes_per_sec = speed;
                        it.segment_map = seg_bar;
                    }
                }
            }
        });

        // Spawn parallel range download workers
        let mut handles = Vec::with_capacity(threads);
        for i in 0..threads {
            let start_byte = (i as u64) * chunk_size;
            let end_byte = if i == threads - 1 {
                total_len - 1
            } else {
                start_byte + chunk_size - 1
            };
            let current_done = thread_progress[i].load(Ordering::SeqCst);
            let mut current_offset = start_byte + current_done;

            let client_clone = client.clone();
            let item_clone = item.clone();
            let file_clone = shared_file.clone();
            let prog_clone = thread_progress.clone();
            let run_clone = running.clone();
            let limit_clone = speed_limit.clone();
            let offset_path = QueueStorage::parts_dir().join(format!("{}_t{}.offset", item.id, i));

            handles.push(tokio::spawn(async move {
                if current_offset > end_byte {
                    return Ok::<(), String>(());
                }

                let resp = apply_headers(client_clone.get(&item_clone.url), &item_clone)
                    .header(RANGE, format!("bytes={}-{}", current_offset, end_byte))
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;

                if !resp.status().is_success() {
                    return Err(format!("HTTP {}", resp.status()));
                }

                let mut stream = resp.bytes_stream();
                let mut bytes_since_checkpoint = 0usize;

                while let Some(chunk_res) = stream.next().await {
                    if !run_clone.load(Ordering::Relaxed) {
                        let _ = fs::write(&offset_path, current_offset.to_string());
                        return Err("Paused".to_string());
                    }

                    let chunk = chunk_res.map_err(|e| e.to_string())?;
                    let len = chunk.len();
                    if len == 0 {
                        break;
                    }

                    write_all_positioned(&file_clone, &chunk, current_offset)
                        .map_err(|e| e.to_string())?;

                    current_offset += len as u64;
                    prog_clone[i].fetch_add(len as u64, Ordering::Relaxed);
                    bytes_since_checkpoint += len;

                    if bytes_since_checkpoint >= 128 * 1024 {
                        let _ = fs::write(&offset_path, current_offset.to_string());
                        bytes_since_checkpoint = 0;
                    }

                    // Token-bucket speed limiter
                    let max_bps = limit_clone.load(Ordering::Relaxed);
                    if max_bps > 0 {
                        let per_thread_bps = (max_bps / (threads as u64)).max(4096);
                        let sleep_ms = ((len as u64) * 1000) / per_thread_bps;
                        if sleep_ms > 5 {
                            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                        }
                    }
                }

                let _ = fs::write(&offset_path, current_offset.to_string());
                Ok(())
            }));
        }

        let mut first_err: Option<String> = None;
        for h in handles {
            match h.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
                Err(e) => {
                    if first_err.is_none() {
                        first_err = Some(e.to_string());
                    }
                }
            }
        }

        monitor_handle.abort();

        if let Some(err) = first_err {
            return Err(err);
        }

        // Remove temporary thread offset files upon completion
        for i in 0..threads {
            let offset_path = QueueStorage::parts_dir().join(format!("{}_t{}.offset", item.id, i));
            let _ = fs::remove_file(offset_path);
        }
    } else {
        // Single-stream fallback
        let resp = apply_headers(client.get(&item.url), item)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }

        let total = resp.content_length().map(|v| v as i64).unwrap_or(-1);
        update_item(shared_queue, item.id, |it| it.total_bytes = total);

        let mut out_file = File::create(part_file).map_err(|e| e.to_string())?;
        let mut stream = resp.bytes_stream();
        let mut downloaded = 0u64;
        let mut last_tick = Instant::now();
        let mut bytes_since_tick = 0u64;

        while let Some(chunk_res) = stream.next().await {
            if !running.load(Ordering::Relaxed) {
                return Err("Paused".to_string());
            }
            let chunk = chunk_res.map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut out_file, &chunk).map_err(|e| e.to_string())?;
            downloaded += chunk.len() as u64;
            bytes_since_tick += chunk.len() as u64;

            let elapsed = last_tick.elapsed().as_millis() as u64;
            if elapsed >= 500 {
                let speed = (bytes_since_tick * 1000) / elapsed.max(1);
                last_tick = Instant::now();
                bytes_since_tick = 0;
                update_item(shared_queue, item.id, |it| {
                    it.downloaded_bytes = downloaded;
                    it.speed_bytes_per_sec = speed;
                });
            }
        }

        update_item(shared_queue, item.id, |it| {
            it.downloaded_bytes = downloaded;
            if it.total_bytes <= 0 {
                it.total_bytes = downloaded as i64;
            }
        });
    }

    finalize_completed_file(item, part_file, &item.file_name, shared_queue)
}

async fn download_hls_stream(
    client: &Client,
    item: &DownloadItem,
    part_file: &Path,
    running: &Arc<AtomicBool>,
    shared_queue: &Arc<Mutex<Vec<DownloadItem>>>,
) -> Result<(), String> {
    let mut manifest = apply_headers(client.get(&item.url), item)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let mut base_url = item
        .url
        .rfind('/')
        .map(|idx| item.url[..=idx].to_string())
        .unwrap_or_default();

    let mut lines: Vec<String> = manifest
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if lines
        .iter()
        .any(|l| l.to_ascii_uppercase().starts_with("#EXT-X-STREAM-INF"))
    {
        if let Some(variant) = lines.iter().rev().find(|l| !l.starts_with('#')).cloned() {
            let resolved = if variant.starts_with("http") {
                variant
            } else {
                format!("{}{}", base_url, variant)
            };
            manifest = apply_headers(client.get(&resolved), item)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .text()
                .await
                .map_err(|e| e.to_string())?;
            if let Some(idx) = resolved.rfind('/') {
                base_url = resolved[..=idx].to_string();
            }
            lines = manifest
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
        }
    }

    let segments: Vec<String> = lines
        .into_iter()
        .filter(|l| !l.starts_with('#'))
        .map(|l| {
            if l.starts_with("http") {
                l
            } else {
                format!("{}{}", base_url, l)
            }
        })
        .collect();

    if segments.is_empty() {
        return Err("No media segments found in M3U8 playlist".to_string());
    }

    let mut out_file = File::create(part_file).map_err(|e| e.to_string())?;
    let mut downloaded = 0u64;

    for (idx, seg_url) in segments.iter().enumerate() {
        if !running.load(Ordering::Relaxed) {
            return Err("Paused".to_string());
        }
        let seg_bytes = apply_headers(client.get(seg_url), item)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;

        std::io::Write::write_all(&mut out_file, &seg_bytes).map_err(|e| e.to_string())?;
        downloaded += seg_bytes.len() as u64;
        let est_total = (downloaded * (segments.len() as u64)) / ((idx + 1) as u64);

        update_item(shared_queue, item.id, |it| {
            it.downloaded_bytes = downloaded;
            it.total_bytes = est_total as i64;
            it.segment_map = format!("[HLS {}/{}]", idx + 1, segments.len());
        });
    }

    let final_name = if item.file_name.to_ascii_lowercase().ends_with(".m3u8") {
        format!("{}.mp4", &item.file_name[..item.file_name.len() - 5])
    } else {
        item.file_name.clone()
    };

    finalize_completed_file(item, part_file, &final_name, shared_queue)
}

fn finalize_completed_file(
    item: &DownloadItem,
    part_file: &Path,
    final_name: &str,
    shared_queue: &Arc<Mutex<Vec<DownloadItem>>>,
) -> Result<(), String> {
    let target_dir = QueueStorage::target_folder(&item.category);
    let mut dest_path: PathBuf = target_dir.join(final_name);

    if dest_path.exists() {
        let stem = dest_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = dest_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("bin");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() % 100_000)
            .unwrap_or(1);
        dest_path = target_dir.join(format!("{}_{}.{}", stem, stamp, ext));
    }

    if fs::rename(part_file, &dest_path).is_err() {
        fs::copy(part_file, &dest_path).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(part_file);
    }

    // Compute SHA-256 checksum
    let mut hasher = Sha256::new();
    if let Ok(mut f) = File::open(&dest_path) {
        let mut buf = [0u8; 64 * 1024];
        while let Ok(n) = f.read(&mut buf) {
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
    }
    let sha_hex = hex::encode(hasher.finalize());
    let final_size = fs::metadata(&dest_path).map(|m| m.len()).unwrap_or(0);
    let saved_path_str = dest_path.to_string_lossy().to_string();

    update_item(shared_queue, item.id, |it| {
        it.total_bytes = final_size as i64;
        it.downloaded_bytes = final_size;
        it.speed_bytes_per_sec = 0;
        it.saved_file_path = Some(saved_path_str);
        it.sha256_hash = Some(sha_hex);
        it.status = "COMPLETED".to_string();
    });

    Ok(())
}
