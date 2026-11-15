# Shunno Get for Windows — Native Rust Edition (শূন্য গেট)

**Shunno Get (Rust Edition)** is an ultra-lightweight (~6 MB), zero-bloat, 32-thread desktop download manager for Windows 10 & 11 built in pure **Rust** (`tokio` + `reqwest` + `eframe`/`egui`) with an Apple Cupertino UI.

## 🦀 Why the Rust Edition?
- **Tiny ~6 MB Single-File `.exe`:** Compiles directly to native x86_64 machine code (`ShunnoGet.exe`) with zero `.NET`, Java, or WebView/Electron runtime required.
- **Ultra-Low Memory Footprint (<20 MB RAM):** Uses OS-level positioned file writes (`FileExt::seek_write`) so all 32 concurrent Tokio streams write directly to their byte offsets on disk without lock contention or memory bloat.
- **Zero-Dependency Native TLS:** Uses Windows SChannel via `reqwest` (`native-tls`) for instant TLS handshakes.

## 💻 1-Click Direct Download
- **[👉 Download ShunnoGet.exe (Standalone Native Rust EXE)](https://github.com/asanaim/ShunnoGet-Windows/releases/latest/download/ShunnoGet.exe)**

## ✨ Key Features
1. **Up to 32 Parallel HTTP Range Threads (`src/engine.rs`):**
   - Probes servers via `HEAD` and fallback ranged `GET (bytes=0-0)`, pre-allocates the `.part` file, and spawns up to 32 concurrent Tokio tasks writing via `FileExt::seek_write` with a live visual segment map (`[████▓▓▒▒]`).
2. **True Pause, Resume & Expired Link Repair (`Fix Link`):**
   - Checkpoints per-thread byte offsets in `%LocalAppData%\ShunnoGet\parts` and saves queue state in `shunnoget_queue.json`.
3. **HLS (`.m3u8`) Stream Grabber & Lossless Audio (`FLAC` / `ALAC`):**
   - Parses `.m3u8` master/media playlists, downloads the highest-bitrate variant stream into `.mp4`, and routes `.flac`, `.m4a`, `.wav`, `.mp4`, `.zip`, and `.exe` files into `Downloads\ShunnoGet\<Category>`.
4. **Live Clipboard Sniffer, Browser Extension & SHA-256 Checksums:**
   - Automatically captures copied download URLs from your Windows clipboard, listens on `http://127.0.0.1:45780/add` for the included Chrome/Edge/Brave extension (`browser-extension/`), expands `[01-50]` wildcard batches, throttles bandwidth, and computes a **SHA-256** digest for every finished download.
