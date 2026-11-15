# Shunno Get for Windows (শূন্য গেট)

**Shunno Get for Windows** is a fast, 100% free, open-source, multi-threaded desktop download manager modeled after Internet Download Manager (IDM) and wrapped in a clean Apple-inspired Cupertino UI.

## 💻 1-Click Direct Download for Windows 10 & 11
- **[👉 Download ShunnoGet-Portable.exe (Standalone Single-File EXE)](https://github.com/asanaim/ShunnoGet-Windows/releases/latest/download/ShunnoGet-Portable.exe)**
- **[Download ShunnoGet-Light.exe (Ultra-Small, requires .NET 8 Desktop Runtime)](https://github.com/asanaim/ShunnoGet-Windows/releases/latest/download/ShunnoGet-Light.exe)**

---

## ✨ Key Features
1. **Up to 32 Parallel HTTP Range Threads (`MultiThreadEngine.cs`):**
   - Probes servers via `HEAD` and ranged `GET(0-0)`, pre-allocates the file on disk, and splits downloads across up to **32 concurrent byte-range threads** writing directly via thread-safe `RandomAccess.WriteAsync`.
   - Includes a live visual segment map (`[████▓▓▒▒]`) showing individual chunk progress in real time.
2. **True Byte-Offset Pause, Resume & Expired Link Repair (`Fix Link`):**
   - Saves thread byte offsets in `%LocalAppData%\ShunnoGet\parts` and persists your queue in `shunnoget_queue.json`.
   - Click **Fix Link** on any paused or interrupted download to paste a fresh CDN URL and resume without losing already-downloaded bytes.
3. **HLS (`.m3u8`) Stream Grabber & Lossless Audio (`FLAC` / `ALAC`):**
   - Automatically parses `.m3u8` master and media playlists, selects the highest-bitrate stream variant, and merges segments into `.mp4`.
   - Preserves bit-perfect `.flac`, `.m4a` (ALAC), `.wav`, `.mp4`, `.zip`, `.iso`, and `.exe` files neatly organized into `Downloads\ShunnoGet\<Category>`.
4. **Live Clipboard Sniffer & Browser Extension Integration:**
   - **Auto-Sniff Clipboard:** Automatically detects copied download URLs (`.zip`, `.mp4`, `.flac`, `.exe`, `.iso`, `.pdf`) on your Windows clipboard.
   - **Browser Extension (`browser-extension/`):** Load the unpacked extension in Chrome, Edge, or Brave to right-click any link or media stream and select **"⚡ Download with Shunno Get"** (passes `Referer` headers directly to `http://127.0.0.1:45780/add`).
5. **Wildcard Batch Grabber, Speed Limiter & SHA-256 Verifier:**
   - Expand sequential URLs with `[01-50]` syntax or import `.txt` link lists, throttle bandwidth (`10 MB/s`, `5 MB/s`, `2 MB/s`, `512 KB/s`), and automatically verify every completed download with a **SHA-256** checksum.
