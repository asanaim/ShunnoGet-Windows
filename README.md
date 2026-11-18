# Shunno Browser for Windows — Full-Featured Native Rust Edition (শূন্য ব্রাউজার)

**Shunno Browser** is a featherweight (~5.5 MB), zero-telemetry desktop browser and 32-thread download manager for Windows 10 & 11 built in pure **Rust** (`tao` + `wry` Edge WebView2 + `tokio` + `reqwest`) with an Apple Cupertino UI.

## 💻 1-Click Direct Download
- **[👉 Download ShunnoBrowser.exe](https://github.com/asanaim/ShunnoBrowser-Windows/releases/latest/download/ShunnoBrowser.exe)**

## ✨ Complete Feature Set Included
1. **Multi-Tab Cupertino Browser + Incognito Mode + Session Restore:**
   - Independent Edge WebView2 tabs (`+ Tab`, `🕵 Private`), automatic session persistence (`session.json`), and pop-up window blocking.
2. **Brave-Style Background YouTube Audio & SponsorBlock:**
   - Overrides the Page Visibility API (`document.hidden = false`) so YouTube and media streams keep playing seamlessly in background tabs or when minimized, paired with YouTube Shorts/ad-shelf removal and automatic video ad skipping.
3. **Naked Browser Pro (NBPro) Per-Site Granular Whitelists (`⚙️ Site`):**
   - Toggle **AdBlock**, **JavaScript**, **Page Images**, **Desktop User-Agent**, **True OLED Dark Mode (`#000000`)**, **SponsorBlock**, and **Auto-Reject Cookie Banners** per domain (`site_policies.json`).
4. **Power-User Reading & Browsing Tools:**
   - **Universal HTML5 Video Speed (`⏩ 0.75× – 4.0×`)**, **Clean Cupertino Reader Mode (`📖 Reader`)**, **Find-in-Page (`🔍 Find`)**, **Tracking Parameter Stripper (`utm_*`, `fbclid`, `gclid`, `si`)**, **Custom User-Scripts (`Downloads\ShunnoBrowser\scripts\*.js`)**, and **Portable `.txt` Bookmarks + History (`⭐ Library`)**.
5. **Integrated 32-Thread Shunno Get Engine (`⚡ Get`):**
   - Intercepts browser downloads and sniffed media streams (`.mp4`, `.m3u8` HLS, `.flac`, `.m4a` ALAC, `.zip`, `.iso`, `.exe`) across up to **32 parallel HTTP Range threads** (`FileExt::seek_write`), with byte-offset Pause/Resume, **Fix Link** URL repair, **Speed Limiter**, **Wildcard Batch `[01-50]`**, and **SHA-256** verification.
