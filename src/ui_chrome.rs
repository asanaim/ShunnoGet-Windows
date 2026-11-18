pub const CHROME_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
  :root {
    --bg: #F2F2F7;
    --surface: #FFFFFF;
    --border: #E5E5EA;
    --blue: #0A84FF;
    --blue-soft: #EBF4FF;
    --green: #30D158;
    --green-soft: #E8F8EE;
    --orange: #FF9F0A;
    --red: #FF3B30;
    --text: #1C1C1E;
    --muted: #8E8E93;
  }
  * { box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI Variable Display", "Segoe UI", sans-serif; user-select: none; }
  body { background: var(--bg); color: var(--text); overflow: hidden; display: flex; flex-direction: column; height: 100vh; }

  /* Tab Strip */
  .tab-bar {
    display: flex; align-items: center; gap: 6px; padding: 6px 10px 4px 10px;
    background: #E5E5EA; border-bottom: 1px solid #D1D1D6; overflow-x: auto; height: 38px;
  }
  .tab-bar::-webkit-scrollbar { display: none; }
  .brand-pill {
    display: flex; align-items: center; gap: 5px; font-weight: 700; font-size: 12px;
    background: linear-gradient(135deg, #0A84FF, #0040DD); color: #fff;
    padding: 4px 10px; border-radius: 9px; flex-shrink: 0;
  }
  .tab {
    display: flex; align-items: center; gap: 6px; padding: 4px 10px; border-radius: 9px;
    background: rgba(255,255,255,0.55); color: #3A3A3C; font-size: 12px; cursor: pointer;
    max-width: 190px; min-width: 95px; transition: background 0.15s; flex-shrink: 0;
  }
  .tab.active { background: #FFFFFF; color: #000; font-weight: 600; box-shadow: 0 1px 4px rgba(0,0,0,0.08); }
  .tab.incognito.active { background: #1C1C1E; color: #FFFFFF; }
  .tab-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
  .tab-close { font-size: 11px; color: var(--muted); padding: 1px 4px; border-radius: 4px; }
  .tab-close:hover { background: #FFEBEA; color: var(--red); }
  .tab-add {
    border: none; background: rgba(255,255,255,0.75); color: var(--blue); font-weight: 700;
    font-size: 13px; padding: 3px 9px; border-radius: 8px; cursor: pointer; flex-shrink: 0;
  }
  .tab-add:hover { background: #fff; }

  /* Main Cupertino Navigation & Power Toolbar */
  .nav-bar {
    display: flex; align-items: center; gap: 6px; padding: 6px 10px;
    background: var(--surface); border-bottom: 1px solid var(--border); height: 48px;
  }
  .nav-btn {
    border: none; background: var(--bg); color: var(--text); font-size: 14px; font-weight: 600;
    padding: 5px 9px; border-radius: 9px; cursor: pointer; display: inline-flex; align-items: center; gap: 4px;
    white-space: nowrap; transition: all 0.12s;
  }
  .nav-btn:hover { background: #E5E5EA; }
  .nav-btn.on { background: var(--blue-soft); color: var(--blue); }
  .nav-btn.green { background: var(--green-soft); color: #1E9E42; }
  .url-wrap {
    flex: 1; display: flex; align-items: center; background: var(--bg);
    border-radius: 11px; padding: 0 10px; height: 34px; border: 1px solid transparent;
  }
  .url-wrap:focus-within { background: #fff; border-color: var(--blue); box-shadow: 0 0 0 3px rgba(10,132,255,0.15); }
  #url-input {
    border: none; background: transparent; outline: none; width: 100%;
    font-size: 13px; color: var(--text); user-select: text;
  }

  /* Expandable Drawer Area */
  .drawer {
    flex: 1; background: var(--bg); padding: 14px 18px; overflow-y: auto; display: none;
  }
  .drawer.open { display: block; }
  .card {
    background: #fff; border-radius: 14px; padding: 14px 16px; margin-bottom: 12px;
    border: 1px solid var(--border); box-shadow: 0 2px 8px rgba(0,0,0,0.03);
  }
  .card-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
  .card-title { font-size: 15px; font-weight: 700; }
  .grid-toggles { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 10px; }
  .toggle-tile {
    display: flex; justify-content: space-between; align-items: center;
    padding: 10px 12px; border-radius: 11px; background: var(--bg); font-size: 12.5px; font-weight: 600;
  }
  .pill-btn {
    border: none; border-radius: 8px; padding: 5px 11px; font-size: 12px; font-weight: 600; cursor: pointer;
  }
  .pill-blue { background: var(--blue); color: #fff; }
  .pill-soft { background: var(--blue-soft); color: var(--blue); }
  .pill-green { background: var(--green-soft); color: #1E9E42; }
  .pill-orange { background: #FFF5E5; color: var(--orange); }
  .pill-red { background: #FFEBEA; color: var(--red); }

  .input-row { display: flex; gap: 8px; margin-bottom: 10px; }
  .text-box {
    flex: 1; padding: 8px 12px; border-radius: 10px; border: 1px solid #D1D1D6;
    font-size: 13px; outline: none; user-select: text;
  }
  .select-box { padding: 8px 10px; border-radius: 10px; border: 1px solid #D1D1D6; font-size: 12.5px; background: #fff; }

  .prog-track { width: 100%; height: 7px; background: #E5E5EA; border-radius: 4px; overflow: hidden; margin: 7px 0; }
  .prog-fill { height: 100%; background: var(--blue); transition: width 0.2s; }
  .prog-fill.done { background: var(--green); }
  .meta { font-size: 11.5px; color: var(--muted); }
  .list-row {
    display: flex; justify-content: space-between; align-items: center; padding: 8px 0;
    border-bottom: 1px solid var(--bg); gap: 10px;
  }
  .list-row:last-child { border-bottom: none; }
</style>
</head>
<body>

  <!-- 1. Cupertino Tab Bar -->
  <div class="tab-bar" id="tab-bar">
    <div class="brand-pill">0 Shunno</div>
    <div id="tabs-container" style="display:flex;gap:6px;align-items:center;"></div>
    <button class="tab-add" onclick="send('NEW_TAB')" title="New Tab (Ctrl+T)">+ Tab</button>
    <button class="tab-add" style="color:#1C1C1E" onclick="send('NEW_INCOGNITO')" title="New Private Incognito Tab">🕵 Private</button>
  </div>

  <!-- 2. Navigation & NBPro / Brave / Shunno Get Power Toolbar -->
  <div class="nav-bar">
    <button class="nav-btn" onclick="send('BACK')" title="Back">‹</button>
    <button class="nav-btn" onclick="send('FORWARD')" title="Forward">›</button>
    <button class="nav-btn" onclick="send('RELOAD')" title="Reload">↻</button>

    <div class="url-wrap">
      <input id="url-input" placeholder="Search DuckDuckGo or enter URL (tracking parameters auto-stripped)..."
             onkeydown="if(event.key==='Enter') send('NAV:' + this.value)" />
    </div>

    <button class="nav-btn green" id="btn-streams" style="display:none" onclick="toggleDrawer('streams')">🎯 Streams (0)</button>
    <button class="nav-btn on" id="btn-bgaudio" onclick="send('TOGGLE_BG_AUDIO')" title="Brave-Style Background YouTube Audio">🎧 BG Audio</button>
    <button class="nav-btn on" id="btn-adblock" onclick="send('TOGGLE_SITE:adblock')" title="Network + DOM AdBlock & SponsorBlock">🛡️ AdBlock</button>
    <button class="nav-btn" id="btn-speed" onclick="cycleSpeed()" title="HTML5 Video/Audio Playback Speed">⏩ 1.0×</button>
    <button class="nav-btn" onclick="send('READER_MODE')" title="Clean Cupertino Reader View">📖 Reader</button>
    <button class="nav-btn" id="btn-oled" onclick="send('TOGGLE_SITE:oled_dark')" title="Per-Site True OLED Dark Mode">🌑 OLED</button>
    <button class="nav-btn" onclick="toggleDrawer('find')" title="Find in Page">🔍 Find</button>
    <button class="nav-btn" onclick="toggleDrawer('site')" title="NBPro Per-Site Controls & UserScripts">⚙️ Site</button>
    <button class="nav-btn" onclick="toggleDrawer('bookmarks')" title="Bookmarks, History & Snapshot">⭐ Library</button>
    <button class="nav-btn on" id="btn-downloads" onclick="toggleDrawer('downloads')" title="Shunno Get 32-Thread Download Manager">⚡ Get (0)</button>
  </div>

  <!-- 3. Expandable Drawer Overlay -->
  <div class="drawer" id="drawer">

    <!-- Drawer A: Find in Page -->
    <div id="panel-find" style="display:none">
      <div class="card">
        <div class="card-header">
          <span class="card-title">🔍 Find in Page</span>
          <button class="pill-btn pill-red" onclick="closeDrawer()">✕ Close</button>
        </div>
        <div class="input-row">
          <input class="text-box" id="find-input" placeholder="Type text to highlight on active page..."
                 oninput="send('FIND:' + this.value)" onkeydown="if(event.key==='Enter') send('FIND:' + this.value)" />
          <button class="pill-btn pill-soft" onclick="send('FIND_PREV')">↑ Prev</button>
          <button class="pill-btn pill-blue" onclick="send('FIND_NEXT')">↓ Next</button>
        </div>
      </div>
    </div>

    <!-- Drawer B: Sniffed Media & Streams (HLS .m3u8 / FLAC / ALAC / MP4) -->
    <div id="panel-streams" style="display:none">
      <div class="card">
        <div class="card-header">
          <span class="card-title">🎯 Sniffed Media & Lossless Audio Streams</span>
          <button class="pill-btn pill-red" onclick="closeDrawer()">✕ Close</button>
        </div>
        <div id="streams-list"></div>
      </div>
    </div>

    <!-- Drawer C: NBPro Per-Site Controls, Global Privacy & UserScripts -->
    <div id="panel-site" style="display:none">
      <div class="card">
        <div class="card-header">
          <span class="card-title" id="site-domain-title">⚙️ Per-Site NBPro Whitelist Controls</span>
          <button class="pill-btn pill-red" onclick="closeDrawer()">✕ Close</button>
        </div>
        <div class="grid-toggles" id="site-toggles-grid"></div>
      </div>

      <div class="card">
        <div class="card-header">
          <span class="card-title">🛡️ Global Privacy & Tampermonkey-Lite UserScripts</span>
          <button class="pill-btn pill-soft" onclick="send('OPEN_SCRIPTS_FOLDER')">📂 Open scripts/*.js Folder</button>
        </div>
        <div class="grid-toggles" id="global-toggles-grid"></div>
      </div>
    </div>

    <!-- Drawer D: Portable Bookmarks, History & Offline Page Snapshot -->
    <div id="panel-bookmarks" style="display:none">
      <div class="card">
        <div class="card-header">
          <span class="card-title">⭐ Portable Bookmarks & Offline Snapshot</span>
          <div style="display:flex;gap:6px;">
            <button class="pill-btn pill-blue" onclick="send('ADD_BOOKMARK')">+ Bookmark Page</button>
            <button class="pill-btn pill-green" onclick="send('SAVE_OFFLINE_HTML')">💾 Save Page Offline</button>
            <button class="pill-btn pill-soft" onclick="send('EXPORT_TXT_BOOKMARKS')">Export .TXT</button>
            <button class="pill-btn pill-soft" onclick="send('IMPORT_TXT_BOOKMARKS')">Import .TXT</button>
            <button class="pill-btn pill-red" onclick="closeDrawer()">✕ Close</button>
          </div>
        </div>
        <div id="bookmarks-list"></div>
      </div>

      <div class="card">
        <div class="card-header">
          <span class="card-title">🕒 Recent Browsing History</span>
          <button class="pill-btn pill-red" onclick="send('CLEAR_HISTORY')">Clear History</button>
        </div>
        <div id="history-list"></div>
      </div>
    </div>

    <!-- Drawer E: Shunno Get 32-Thread Download Manager -->
    <div id="panel-downloads" style="display:none">
      <div class="card">
        <div class="card-header">
          <span class="card-title">⚡ Shunno Get — 32-Thread Accelerated Download Engine</span>
          <div style="display:flex;gap:8px;align-items:center;">
            <span class="meta">Speed Limit:</span>
            <select class="select-box" id="speed-limit-select" onchange="send('SET_SPEED_LIMIT:' + this.value)">
              <option value="0">Unlimited</option>
              <option value="10485760">10 MB/s</option>
              <option value="5242880">5 MB/s</option>
              <option value="2097152">2 MB/s</option>
              <option value="524288">512 KB/s</option>
            </select>
            <button class="pill-btn pill-soft" onclick="send('OPEN_DOWNLOADS_FOLDER')">📂 Open Folder</button>
            <button class="pill-btn pill-red" onclick="closeDrawer()">✕ Close</button>
          </div>
        </div>
        <div class="input-row">
          <input class="text-box" id="dl-url-input" placeholder="Paste HTTP/HTTPS link, .flac/.mp4/.m3u8 stream, or wildcard track_[01-12].flac..." />
          <select class="select-box" id="dl-threads-select">
            <option value="1">1 Thread</option>
            <option value="4">4 Threads</option>
            <option value="8">8 Threads</option>
            <option value="16" selected>16 Threads</option>
            <option value="32">32 Threads</option>
          </select>
          <button class="pill-btn pill-blue" onclick="startManualDownload()">⚡ Start Download</button>
        </div>
      </div>

      <div id="downloads-list"></div>
    </div>

  </div>

<script>
  let activeDrawer = null;
  let currentSpeed = 1.0;
  const speeds = [1.0, 1.25, 1.5, 2.0, 3.0, 4.0, 0.75];

  function send(msg) {
    if (window.ipc) window.ipc.postMessage(msg);
  }

  function toggleDrawer(name) {
    if (activeDrawer === name) {
      closeDrawer();
      return;
    }
    activeDrawer = name;
    document.getElementById('drawer').classList.add('open');
    ['find', 'streams', 'site', 'bookmarks', 'downloads'].forEach(p => {
      document.getElementById('panel-' + p).style.display = (p === name) ? 'block' : 'none';
    });
    send('DRAWER_OPEN:1');
  }

  function closeDrawer() {
    activeDrawer = null;
    document.getElementById('drawer').classList.remove('open');
    send('DRAWER_OPEN:0');
  }

  function cycleSpeed() {
    const idx = (speeds.indexOf(currentSpeed) + 1) % speeds.length;
    currentSpeed = speeds[idx];
    document.getElementById('btn-speed').textContent = '⏩ ' + currentSpeed + '×';
    send('SET_PLAYBACK_SPEED:' + currentSpeed);
  }

  function startManualDownload() {
    const input = document.getElementById('dl-url-input');
    const threads = document.getElementById('dl-threads-select').value;
    if (!input.value.trim()) return;
    send('ENQUEUE_DL:' + threads + '|' + input.value.trim());
    input.value = '';
  }

  function fmtBytes(b) {
    if (!b || b <= 0) return '0 B';
    const k = b / 1024, m = k / 1024, g = m / 1024;
    if (g >= 1) return g.toFixed(2) + ' GB';
    if (m >= 1) return m.toFixed(1) + ' MB';
    if (k >= 1) return k.toFixed(0) + ' KB';
    return b + ' B';
  }

  window.renderChromeState = function(state) {
    // Tabs
    const tc = document.getElementById('tabs-container');
    tc.innerHTML = state.tabs.map((t, idx) => `
      <div class="tab ${idx === state.active_tab ? 'active' : ''} ${t.incognito ? 'incognito' : ''}"
           onclick="send('SWITCH_TAB:${idx}')">
        <span class="tab-title">${t.incognito ? '🕵 ' : ''}${escapeHtml(t.title || 'New Tab')}</span>
        <span class="tab-close" onclick="event.stopPropagation(); send('CLOSE_TAB:${idx}')">✕</span>
      </div>
    `).join('');

    const urlBox = document.getElementById('url-input');
    if (document.activeElement !== urlBox) {
      urlBox.value = state.active_url || '';
    }

    // Toolbar badges
    const sBtn = document.getElementById('btn-streams');
    if (state.streams.length > 0) {
      sBtn.style.display = 'inline-flex';
      sBtn.textContent = `🎯 Streams (${state.streams.length})`;
    } else {
      sBtn.style.display = 'none';
    }

    document.getElementById('btn-bgaudio').className = 'nav-btn ' + (state.global.background_youtube_play ? 'on' : '');
    document.getElementById('btn-adblock').className = 'nav-btn ' + (state.site.adblock ? 'on' : '');
    document.getElementById('btn-adblock').textContent = `🛡️ AdBlock (${state.blocked_count})`;
    document.getElementById('btn-oled').className = 'nav-btn ' + (state.site.oled_dark ? 'on' : '');
    document.getElementById('btn-downloads').textContent = `⚡ Get (${state.downloads.length})`;
    document.getElementById('speed-limit-select').value = String(state.global.speed_limit_bps);

    // Streams panel
    document.getElementById('streams-list').innerHTML = state.streams.length === 0
      ? '<div class="meta">No media streams intercepted on this page yet.</div>'
      : state.streams.map(u => `
          <div class="list-row">
            <div style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:12.5px;">${escapeHtml(u)}</div>
            <button class="pill-btn pill-green" onclick="send('ENQUEUE_DL:16|${encodeAttr(u)}'); toggleDrawer('downloads');">⚡ Grab (16T)</button>
          </div>
        `).join('');

    // Site toggles
    document.getElementById('site-domain-title').textContent = `⚙️ Per-Site NBPro Whitelist: ${state.active_host || 'Global'}`;
    const siteItems = [
      ['adblock', '🛡️ Ad & Tracker Blocker', state.site.adblock],
      ['js_enabled', '⚙️ JavaScript Execution', state.site.js_enabled],
      ['images_enabled', '🖼️ Load Page Images', state.site.images_enabled],
      ['desktop_ua', '💻 Desktop User-Agent', state.site.desktop_ua],
      ['oled_dark', '🌑 True OLED Dark (#000)', state.site.oled_dark],
      ['sponsorblock', '🎬 SponsorBlock & Shorts Clean', state.site.sponsorblock],
      ['auto_reject_cookies', '🍪 Auto-Reject Cookie Banners', state.site.auto_reject_cookies]
    ];
    document.getElementById('site-toggles-grid').innerHTML = siteItems.map(([key, label, val]) => `
      <div class="toggle-tile">
        <span>${label}</span>
        <button class="pill-btn ${val ? 'pill-blue' : 'pill-soft'}" onclick="send('TOGGLE_SITE:${key}')">${val ? 'ON' : 'OFF'}</button>
      </div>
    `).join('');

    // Global toggles
    const globalItems = [
      ['bg_audio', '🎧 Brave Background YouTube Audio', state.global.background_youtube_play],
      ['strip_tracking', '✂️ Strip URL Trackers (utm/fbclid)', state.global.strip_tracking_params],
      ['block_popups', '🚫 Block Pop-Up Windows', state.global.block_popups]
    ];
    document.getElementById('global-toggles-grid').innerHTML = globalItems.map(([key, label, val]) => `
      <div class="toggle-tile">
        <span>${label}</span>
        <button class="pill-btn ${val ? 'pill-blue' : 'pill-soft'}" onclick="send('TOGGLE_GLOBAL:${key}')">${val ? 'ON' : 'OFF'}</button>
      </div>
    `).join('');

    // Bookmarks & History
    document.getElementById('bookmarks-list').innerHTML = state.bookmarks.map((b, i) => `
      <div class="list-row">
        <div style="overflow:hidden;">
          <div style="font-size:13px;font-weight:600;">${escapeHtml(b.title)}</div>
          <div class="meta">${escapeHtml(b.url)}</div>
        </div>
        <div style="display:flex;gap:6px;flex-shrink:0;">
          <button class="pill-btn pill-soft" onclick="send('NAV:${encodeAttr(b.url)}'); closeDrawer();">Open</button>
          <button class="pill-btn pill-red" onclick="send('DEL_BOOKMARK:${i}')">✕</button>
        </div>
      </div>
    `).join('');

    document.getElementById('history-list').innerHTML = state.history.length === 0
      ? '<div class="meta">No history recorded yet.</div>'
      : state.history.slice(0, 30).map(h => `
          <div class="list-row">
            <div style="overflow:hidden;">
              <div style="font-size:12.5px;font-weight:600;">${escapeHtml(h.title || h.url)}</div>
              <div class="meta">${escapeHtml(h.url)}</div>
            </div>
            <button class="pill-btn pill-soft" onclick="send('NAV:${encodeAttr(h.url)}'); closeDrawer();">Visit</button>
          </div>
        `).join('');

    // Downloads list
    document.getElementById('downloads-list').innerHTML = state.downloads.length === 0
      ? '<div class="card"><div class="meta">No downloads in queue. Paste a URL above or click any downloadable file on a webpage.</div></div>'
      : state.downloads.map(d => {
          const pct = d.status === 'COMPLETED' ? 100 : (d.total_bytes > 0 ? Math.min(100, (d.downloaded_bytes * 100 / d.total_bytes)) : 0);
          const sizeTxt = d.total_bytes > 0 ? `${fmtBytes(d.downloaded_bytes)} / ${fmtBytes(d.total_bytes)} (${pct.toFixed(1)}%)` : fmtBytes(d.downloaded_bytes);
          const infoTxt = d.status === 'DOWNLOADING'
            ? `⚡ ${fmtBytes(d.speed_bps)}/s ${d.segment_map || ''}`
            : d.status === 'COMPLETED'
              ? `✓ Saved • SHA-256: ${(d.sha256 || '').slice(0, 16)}...`
              : d.status === 'FAILED'
                ? `⚠ ${escapeHtml(d.error_message || 'Error')}`
                : `Paused at ${fmtBytes(d.downloaded_bytes)}`;

          return `
            <div class="card">
              <div class="card-header" style="margin-bottom:4px;">
                <div>
                  <div style="font-size:14px;font-weight:700;">${escapeHtml(d.file_name)}</div>
                  <div class="meta" style="color:#0A84FF;">${escapeHtml(d.category)} • ${d.supports_ranges ? d.threads + ' Threads' : '1 Stream'} • ${d.status}</div>
                </div>
                <div class="meta" style="font-weight:600;">${sizeTxt}</div>
              </div>
              <div class="prog-track"><div class="prog-fill ${d.status === 'COMPLETED' ? 'done' : ''}" style="width:${pct}%"></div></div>
              <div style="display:flex;justify-content:space-between;align-items:center;margin-top:6px;">
                <div class="meta">${infoTxt}</div>
                <div style="display:flex;gap:5px;">
                  ${d.status === 'DOWNLOADING'
                    ? `<button class="pill-btn pill-orange" onclick="send('DL_PAUSE:${d.id}')">Pause</button>`
                    : d.status !== 'COMPLETED'
                      ? `<button class="pill-btn pill-soft" onclick="send('DL_RESUME:${d.id}')">Resume</button>
                         <button class="pill-btn pill-soft" onclick="const u=prompt('Paste fresh mirror/CDN URL to resume from byte offset:','${encodeAttr(d.url)}'); if(u) send('DL_FIX:${d.id}|'+u);">Fix Link</button>`
                      : `<button class="pill-btn pill-green" onclick="send('DL_SHOW:${encodeAttr(d.saved_path || '')}')">Show File</button>`
                  }
                  <button class="pill-btn pill-red" onclick="send('DL_DEL:${d.id}')">✕</button>
                </div>
              </div>
            </div>
          `;
        }).join('');
  };

  function escapeHtml(str) {
    return String(str).replace(/[&<>"']/g, m => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[m]));
  }
  function encodeAttr(str) {
    return String(str).replace(/'/g, "\\'");
  }
</script>
</body>
</html>
"##;
