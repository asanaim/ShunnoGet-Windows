chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: "shunnoget-rust-download",
    title: "⚡ Download with Shunno Get",
    contexts: ["link", "video", "audio"]
  });
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  const targetUrl = info.linkUrl || info.srcUrl;
  if (!targetUrl) return;
  const referer = tab && tab.url ? tab.url : "";
  const endpoint = `http://127.0.0.1:45780/add?url=${encodeURIComponent(targetUrl)}&referer=${encodeURIComponent(referer)}`;
  fetch(endpoint).catch(() => {});
});
