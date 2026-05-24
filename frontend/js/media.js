// ─────────────────────────────────────────
// media.js — Upload, preview, send, render
// ─────────────────────────────────────────

let _pendingFile    = null;
let _pendingCaption = "";

// ── Init ──────────────────────────────────
document.addEventListener("DOMContentLoaded", () => {
  initPasteListener();
  initDragDrop();
});

function initPasteListener() {
  document.addEventListener("paste", (e) => {
    if (!currentRoom || !canRoomBeSent()) return;
    const items = e.clipboardData?.items;
    if (!items) return;
    for (const item of items) {
      if (item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (file) { e.preventDefault(); openMediaPreview(file); }
        return;
      }
    }
  });
}

function initDragDrop() {
  const chatWin = document.getElementById("chat-window");
  if (!chatWin) return;
  chatWin.addEventListener("dragover", (e) => {
    if (!currentRoom || !canRoomBeSent()) return;
    e.preventDefault();
    chatWin.classList.add("drag-over");
  });
  chatWin.addEventListener("dragleave", () => chatWin.classList.remove("drag-over"));
  chatWin.addEventListener("drop", (e) => {
    e.preventDefault();
    chatWin.classList.remove("drag-over");
    if (!currentRoom || !canRoomBeSent()) return;
    const file = e.dataTransfer?.files?.[0];
    if (file) openMediaPreview(file);
  });
}

// ── Toggle: click paperclip → open file picker ──
function toggleMediaPanel() {
  if (editingMessageId) cancelEdit();
  const picker = document.getElementById("media-file");
  if (picker) picker.click();
}

function handleSelectedFile() {
  const file = mediaFile?.files?.[0];
  if (!file) return;
  openMediaPreview(file);
  mediaFile.value = "";
}

// ── Preview modal ──────────────────────────
function openMediaPreview(file) {
  const MAX = 5 * 1024 * 1024;
  if (file.size > MAX) {
    showToast(`File quá lớn! Tối đa ${formatBytes(MAX)}`, "error");
    return;
  }
  _pendingFile    = file;
  _pendingCaption = "";

  let modal = document.getElementById("media-preview-modal");
  if (!modal) modal = _createPreviewModal();

  const previewArea = modal.querySelector("#mp-preview-area");
  const nameEl      = modal.querySelector("#mp-file-name");
  const sizeEl      = modal.querySelector("#mp-file-size");
  const captionEl   = modal.querySelector("#mp-caption");
  const sendBtn     = modal.querySelector("#mp-send-btn");
  const progressEl  = modal.querySelector("#mp-progress");

  previewArea.innerHTML  = "";
  captionEl.value        = "";
  nameEl.textContent     = file.name;
  sizeEl.textContent     = formatBytes(file.size);
  sendBtn.disabled       = false;
  sendBtn.textContent    = "Gửi";
  if (progressEl) { progressEl.style.width = "0%"; progressEl.parentElement.style.display = "none"; }

  if (file.type.startsWith("image/")) {
    const url = URL.createObjectURL(file);
    const img = document.createElement("img");
    img.src   = url;
    img.style.cssText = "max-width:100%;max-height:280px;border-radius:10px;display:block;margin:0 auto;object-fit:contain;";
    img.onload = () => URL.revokeObjectURL(url);
    previewArea.appendChild(img);
  } else if (file.type.startsWith("video/")) {
    const url   = URL.createObjectURL(file);
    const video = document.createElement("video");
    video.src      = url;
    video.controls = true;
    video.style.cssText = "max-width:100%;max-height:240px;border-radius:10px;display:block;margin:0 auto;";
    previewArea.appendChild(video);
  } else {
    previewArea.innerHTML = `
      <div style="display:flex;flex-direction:column;align-items:center;gap:12px;padding:24px 0;">
        <div style="width:56px;height:56px;border-radius:14px;background:var(--accent-soft);
                    display:flex;align-items:center;justify-content:center;">
          <svg viewBox="0 0 24 24" fill="var(--accent)" width="28" height="28">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6zm4 18H6V4h7v5h5v11z"/>
          </svg>
        </div>
        <span style="font-size:13px;color:var(--text-sub);text-align:center;">${escapeHtml(file.name)}</span>
      </div>`;
  }

  modal.classList.remove("hidden");
  setTimeout(() => captionEl?.focus(), 80);
}

function _createPreviewModal() {
  const modal = document.createElement("div");
  modal.id    = "media-preview-modal";
  modal.style.cssText = `
    position:fixed;inset:0;z-index:2000;
    background:rgba(0,0,0,.55);backdrop-filter:blur(6px);
    display:flex;align-items:center;justify-content:center;padding:20px;
  `;
  modal.innerHTML = `
    <div style="
      background:var(--surface);border:1px solid var(--border-solid);
      border-radius:20px;box-shadow:var(--shadow-xl);
      width:min(480px,100%);padding:24px;
      font-family:var(--font-body,system-ui);
      animation:modalIn .22s cubic-bezier(.34,1.56,.64,1) both;
    ">
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:16px;">
        <span style="font-size:16px;font-weight:700;color:var(--text);">Gửi tệp</span>
        <button onclick="closeMediaPreview()"
          style="width:30px;height:30px;border-radius:50%;color:var(--text-sub);
                 display:flex;align-items:center;justify-content:center;
                 font-size:20px;cursor:pointer;transition:background .15s;"
          onmouseover="this.style.background='var(--surface2)'"
          onmouseout="this.style.background=''">&times;</button>
      </div>

      <div id="mp-preview-area" style="
        background:var(--surface2);border-radius:12px;
        padding:12px;margin-bottom:14px;min-height:80px;
      "></div>

      <div style="display:flex;align-items:center;gap:8px;margin-bottom:12px;">
        <div style="flex:1;min-width:0;">
          <div id="mp-file-name" style="font-size:13px;font-weight:600;color:var(--text);
               white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"></div>
          <div id="mp-file-size" style="font-size:12px;color:var(--text-sub);margin-top:2px;"></div>
        </div>
      </div>

      <!-- Progress bar (hidden by default) -->
      <div id="mp-progress-wrap" style="display:none;height:4px;background:var(--surface2);
           border-radius:2px;margin-bottom:12px;overflow:hidden;">
        <div id="mp-progress" style="height:100%;background:var(--accent);border-radius:2px;
             width:0%;transition:width .2s ease;"></div>
      </div>

      <input id="mp-caption" type="text" maxlength="300"
        placeholder="Thêm caption (tùy chọn)..."
        style="width:100%;height:42px;padding:0 14px;
               background:var(--surface2);border:1.5px solid var(--border-solid);
               border-radius:10px;color:var(--text);font-size:14px;
               outline:none;margin-bottom:16px;font-family:inherit;
               transition:border-color .15s;"
        onfocus="this.style.borderColor='var(--accent)'"
        onblur="this.style.borderColor='var(--border-solid)'"
        onkeypress="if(event.key==='Enter'){event.preventDefault();sendPendingMedia();}">

      <div style="display:flex;gap:10px;justify-content:flex-end;">
        <button onclick="closeMediaPreview()"
          style="padding:10px 20px;border-radius:50px;font-size:13px;font-weight:600;
                 background:var(--surface2);color:var(--text-sub);cursor:pointer;
                 border:1.5px solid var(--border-solid);transition:background .15s;"
          onmouseover="this.style.background='var(--surface3)'"
          onmouseout="this.style.background='var(--surface2)'">Hủy</button>
        <button id="mp-send-btn" onclick="sendPendingMedia()"
          style="padding:10px 24px;border-radius:50px;font-size:13px;font-weight:700;
                 background:var(--accent);color:white;cursor:pointer;
                 border:none;transition:opacity .15s;font-family:inherit;
                 box-shadow:0 2px 8px rgba(24,119,242,.3);"
          onmouseover="this.style.opacity='.88'"
          onmouseout="this.style.opacity='1'">Gửi</button>
      </div>
    </div>`;
  modal.addEventListener("click", (e) => { if (e.target === modal) closeMediaPreview(); });
  document.body.appendChild(modal);
  return modal;
}

function closeMediaPreview() {
  const modal = document.getElementById("media-preview-modal");
  if (modal) modal.classList.add("hidden");
  _pendingFile    = null;
  _pendingCaption = "";
}

// ── Send ───────────────────────────────────
function sendPendingMedia() {
  if (!_pendingFile)       { showToast("Chưa chọn file", "warning"); return; }
  if (!currentRoom)        { showToast("Vui lòng chọn phòng trước", "warning"); return; }
  if (!canRoomBeSent())    { showToast("Chat đang bị khoá", "warning"); return; }
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    showToast("Mất kết nối tới server", "error"); return;
  }

  const file    = _pendingFile;
  const caption = document.getElementById("mp-caption")?.value.trim() || "";
  const sendBtn = document.getElementById("mp-send-btn");
  const progressWrap = document.getElementById("mp-progress-wrap");
  const progressBar  = document.getElementById("mp-progress");
  const MAX = 5 * 1024 * 1024;

  if (file.size > MAX) { showToast(`File quá lớn! Tối đa ${formatBytes(MAX)}`, "error"); return; }

  if (sendBtn)       { sendBtn.disabled = true; sendBtn.textContent = "Đang đọc..."; }
  if (progressWrap)  { progressWrap.style.display = "block"; }
  if (progressBar)   { progressBar.style.width = "5%"; }

  const reader = new FileReader();

  reader.onprogress = (e) => {
    if (!e.lengthComputable || !progressBar) return;
    const pct = Math.round((e.loaded / e.total) * 80); // 0-80%
    progressBar.style.width = pct + "%";
    if (sendBtn) sendBtn.textContent = `${pct}%`;
  };

  reader.onload = () => {
    try {
      if (progressBar) progressBar.style.width = "95%";

      const mediaType = detectMediaType(file);
      const payload   = {
        msg_type:   "media",
        username,
        password:   "",
        content:    caption,
        room:       currentRoom,
        target:     isPrivateRoom(currentRoom) ? getPrivateTarget(currentRoom) : "",
        users:      [],
        message_id: "",
        timestamp:  0,
        media: {
          media_type: mediaType,
          file_url:   reader.result,   // base64 data URL
          file_name:  file.name,
          file_size:  file.size,
          mime_type:  file.type || "application/octet-stream"
        }
      };

      ws.send(JSON.stringify(payload));

      if (progressBar) progressBar.style.width = "100%";
      setTimeout(() => {
        closeMediaPreview();
        messageInput?.focus();
        showToast("Đã gửi ảnh!", "success", 2000);
      }, 300);

    } catch (err) {
      console.error("[media] send error:", err);
      showToast("Lỗi khi gửi: " + err.message, "error");
      if (sendBtn) { sendBtn.disabled = false; sendBtn.textContent = "Gửi lại"; }
      if (progressWrap) progressWrap.style.display = "none";
    }
  };

  reader.onerror = (e) => {
    console.error("[media] read error:", e);
    showToast("Không thể đọc file, thử lại", "error");
    if (sendBtn) { sendBtn.disabled = false; sendBtn.textContent = "Gửi"; }
    if (progressWrap) progressWrap.style.display = "none";
  };

  reader.readAsDataURL(file);
}

// Legacy – gọi từ sendMessage khi có file trong input cũ
function sendMedia(caption) {
  const file = mediaFile?.files?.[0];
  if (file) { _pendingFile = file; sendPendingMedia(); mediaFile.value = ""; }
}

// ── Render bubble content ──────────────────
function buildMessageContent(msg) {
  const wrapper = document.createElement("div");

  // Text / caption
  const text = (msg.content || "").trim();
  if (text) {
    const div = document.createElement("div");
    div.appendChild(linkifyContent(text));
    wrapper.appendChild(div);
  }

  // Media — hỗ trợ cả field `media` object lẫn field `file_url` flat
  const mediaObj = _extractMedia(msg);
  if (mediaObj && msg.msg_type !== "deleted") {
    wrapper.appendChild(renderMedia(mediaObj));
  }

  // Nếu không có gì (msg bị xóa)
  if (!text && !mediaObj) {
    const empty = document.createElement("span");
    empty.style.opacity = ".55";
    empty.style.fontStyle = "italic";
    empty.textContent = "[tin nhắn trống]";
    wrapper.appendChild(empty);
  }

  return wrapper;
}

// Extract media object từ nhiều định dạng server có thể trả về
function _extractMedia(msg) {
  if (msg.media && msg.media.file_url) return msg.media;

  // Flat fields (some Rust servers send this way)
  if (msg.file_url) {
    return {
      media_type: msg.media_type || _guessMediaType(msg.file_url, msg.file_name || ""),
      file_url:   msg.file_url,
      file_name:  msg.file_name  || "file",
      file_size:  msg.file_size  || 0,
      mime_type:  msg.mime_type  || "",
    };
  }

  return null;
}

function _guessMediaType(url, name) {
  const lc = (url + name).toLowerCase();
  if (/\.(jpg|jpeg|png|gif|webp|bmp|svg)/.test(lc) || url.startsWith("data:image")) return "image";
  if (/\.(mp4|webm|ogg|mov|avi)/.test(lc)          || url.startsWith("data:video")) return "video";
  return "file";
}

function detectMediaType(file) {
  if (file.type.startsWith("image/")) return "image";
  if (file.type.startsWith("video/")) return "video";
  return "file";
}

// ── Render media in bubble ─────────────────
function renderMedia(media) {
  const box = document.createElement("div");
  box.className = "msg-media";

  if (media.media_type === "image") {
    const img        = document.createElement("img");
    img.src          = media.file_url;
    img.alt          = media.file_name || "ảnh";
    img.loading      = "lazy";
    img.style.cursor = "pointer";
    img.title        = "Nhấn để phóng to";
    img.onerror      = () => { img.style.display = "none"; };
    img.addEventListener("click", () => openImageLightbox(media.file_url, media.file_name || "ảnh"));
    box.appendChild(img);
    if (media.file_name) {
      const info = document.createElement("div");
      info.className   = "msg-meta";
      info.textContent = `${media.file_name}${media.file_size ? " · " + formatBytes(media.file_size) : ""}`;
      box.appendChild(info);
    }

  } else if (media.media_type === "video") {
    const video    = document.createElement("video");
    video.src      = media.file_url;
    video.controls = true;
    video.preload  = "metadata";
    video.style.cssText = "display:block;";
    box.appendChild(video);
    const info = document.createElement("div");
    info.className   = "msg-meta";
    info.textContent = `${media.file_name || "video"}${media.file_size ? " · " + formatBytes(media.file_size) : ""}`;
    box.appendChild(info);

  } else {
    const link     = document.createElement("a");
    link.className = "file-link";
    link.href      = media.file_url;
    link.target    = "_blank";
    link.rel       = "noopener noreferrer";
    link.download  = media.file_name || "file";
    link.innerHTML = `
      <span style="flex-shrink:0;">
        <svg viewBox="0 0 24 24" fill="currentColor" width="18" height="18">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6zm4 18H6V4h7v5h5v11zM8 15h8v2H8zm0-4h8v2H8z"/>
        </svg>
      </span>
      <span style="flex:1;min-width:0;">
        <div class="file-name">${escapeHtml(media.file_name || "file")}</div>
        <div style="font-size:11px;opacity:.6;margin-top:1px;">${media.file_size ? formatBytes(media.file_size) : ""}</div>
      </span>
      <span style="flex-shrink:0;opacity:.5;">
        <svg viewBox="0 0 24 24" fill="currentColor" width="16" height="16">
          <path d="M19 9h-4V3H9v6H5l7 7 7-7zm-7 11l-5-5h3V9h4v6h3l-5 5z" style="display:none"/>
          <path d="M5 20h14v-2H5v2zm7-18l-5 5h3v6h4v-6h3L12 2z" transform="scale(1,-1) translate(0,-24)"/>
          <path d="M19 9h-4V3H9v6H5l7 7 7-7z"/>
        </svg>
      </span>`;
    box.appendChild(link);
  }

  return box;
}

// ── Lightbox ───────────────────────────────
function openImageLightbox(url, name) {
  let lb = document.getElementById("img-lightbox");
  if (!lb) {
    lb = document.createElement("div");
    lb.id = "img-lightbox";
    lb.style.cssText = `
      position:fixed;inset:0;z-index:3000;
      background:rgba(0,0,0,.9);backdrop-filter:blur(8px);
      display:flex;flex-direction:column;align-items:center;justify-content:center;
      padding:20px;cursor:zoom-out;
      animation:fadeIn .15s ease;
    `;
    lb.innerHTML = `
      <div style="position:absolute;top:14px;right:14px;display:flex;gap:8px;z-index:1;">
        <a id="lb-download" download
          style="padding:8px 16px;border-radius:50px;background:rgba(255,255,255,.15);
                 color:white;font-size:13px;font-weight:600;text-decoration:none;
                 border:1px solid rgba(255,255,255,.25);
                 display:flex;align-items:center;gap:6px;backdrop-filter:blur(8px);">
          <svg viewBox="0 0 24 24" fill="currentColor" width="14" height="14">
            <path d="M19 9h-4V3H9v6H5l7 7 7-7zm-7 11l-5-5h3V9h4v6h3l-5 5z" style="display:none"/>
            <path d="M5 20h14v-2H5v2zm7-11v6h2v-6h3l-5-5-5 5h3z" transform="scale(1,-1) translate(0,-24)"/>
            <path d="M19 9h-4V3H9v6H5l7 7 7-7z"/>
          </svg>
          Tải xuống
        </a>
        <button onclick="closeImageLightbox()"
          style="width:36px;height:36px;border-radius:50%;background:rgba(255,255,255,.15);
                 color:white;font-size:22px;cursor:pointer;
                 border:1px solid rgba(255,255,255,.25);backdrop-filter:blur(8px);
                 display:flex;align-items:center;justify-content:center;">&times;</button>
      </div>
      <img id="lb-img"
        style="max-width:90vw;max-height:85vh;border-radius:10px;
               object-fit:contain;box-shadow:0 20px 60px rgba(0,0,0,.6);" alt="">
      <div id="lb-name"
        style="color:rgba(255,255,255,.5);font-size:12px;margin-top:10px;"></div>`;
    lb.addEventListener("click", (e) => { if (e.target === lb) closeImageLightbox(); });
    document.body.appendChild(lb);
  }
  lb.querySelector("#lb-img").src          = url;
  lb.querySelector("#lb-download").href    = url;
  lb.querySelector("#lb-download").download = name;
  lb.querySelector("#lb-name").textContent = name;
  lb.style.display = "flex";
  document.addEventListener("keydown", _lbKey);
}

function closeImageLightbox() {
  const lb = document.getElementById("img-lightbox");
  if (lb) lb.style.display = "none";
  document.removeEventListener("keydown", _lbKey);
}

function _lbKey(e) { if (e.key === "Escape") closeImageLightbox(); }

function mediaFallbackText(msg) {
  const m = _extractMedia(msg);
  return m ? `[${m.media_type}]` : "";
}