// ─────────────────────────────────────────
// ui.js — Sidebar, messages, panels
// ─────────────────────────────────────────

// ══════════════════════════════════════════
// TOAST NOTIFICATION SYSTEM
// Replaces all alert/confirm/prompt calls
// ══════════════════════════════════════════
// ══════════════════════════════════════════
// TOAST + MODAL SYSTEM
// ══════════════════════════════════════════
(function buildToastSystem() {
  const SVG = {
    success: `<svg viewBox="0 0 24 24" fill="currentColor" width="18" height="18"><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/></svg>`,
    error:   `<svg viewBox="0 0 24 24" fill="currentColor" width="18" height="18"><circle cx="12" cy="12" r="10"/><path fill="white" d="M13 7h-2v6h2zm0 8h-2v2h2z"/></svg>`,
    warning: `<svg viewBox="0 0 24 24" fill="currentColor" width="18" height="18"><path d="M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z"/></svg>`,
    info:    `<svg viewBox="0 0 24 24" fill="currentColor" width="18" height="18"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/></svg>`,
  };

  // ── Toast type theme colors ──
  const TYPE_COLORS = {
    success: { bg: '#e8f5e9', text: '#2e7d32', border: '#a5d6a7', icon: '#2e7d32' },
    error:   { bg: '#ffebee', text: '#c62828', border: '#ef9a9a', icon: '#c62828' },
    warning: { bg: '#fff8e1', text: '#f57f17', border: '#ffe082', icon: '#f57f17' },
    info:    { bg: '#e3f2fd', text: '#1565c0', border: '#90caf9', icon: '#1565c0' },
  };

  function getContainer() {
    let el = document.getElementById("toast-container");
    if (!el) {
      el = document.createElement("div");
      el.id = "toast-container";
      document.body.appendChild(el);
    }
    return el;
  }

  // ── Track active toasts by key (message+type) for deduplication ──
  window.__toastMap = window.__toastMap || new Map();
  const MAX_TOASTS = 3;

  window.showToast = function(message, type = "info", duration = 4000) {
    const container = getContainer();
    const toastKey = `${message}::${type}`;

    // ── Deduplication: if same message+type exists, reset its timer ──
    if (window.__toastMap.has(toastKey)) {
      const existing = window.__toastMap.get(toastKey);
      clearTimeout(existing.timer);
      // Refresh progress bar
      const progress = existing.el.querySelector('.toast-progress');
      if (progress) {
        progress.style.animation = 'none';
        void progress.offsetHeight; // trigger reflow
        progress.style.animation = `toastProgress ${duration}ms linear forwards`;
      }
      const newTimer = setTimeout(() => dismissToast(existing.el, toastKey), duration);
      existing.timer = newTimer;
      return existing.el;
    }

    const colors = TYPE_COLORS[type] || TYPE_COLORS.info;
    const isDark = document.body.classList.contains("theme-dark");
    const bg  = isDark ? '#323232' : colors.bg;
    const clr = isDark ? '#e0e0e0' : colors.text;
    const bdr = isDark ? '#424242' : colors.border;

    const toast = document.createElement("div");
    toast.className = `toast-item toast-${type}`;
    toast.style.cssText = `background:${bg};color:${clr};border-color:${bdr};`;
    toast.innerHTML = `
      <span class="toast-icon" style="color:${colors.icon};">${SVG[type] || SVG.info}</span>
      <span class="toast-text">${message}</span>
      <button class="toast-close" aria-label="close">&times;</button>
      <div class="toast-progress" style="animation:toastProgress ${duration}ms linear forwards;background:${colors.icon};"></div>`;

    // ── Close button only (not whole toast) ──
    const closeBtn = toast.querySelector('.toast-close');
    closeBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      dismissToast(toast, toastKey);
    });

    // ── Enforce max stack — remove oldest if at limit ──
    const currentToasts = container.querySelectorAll('.toast-item');
    while (currentToasts.length >= MAX_TOASTS) {
      const oldest = container.firstElementChild;
      if (oldest) {
        const oldKey = findToastKey(oldest);
        dismissToast(oldest, oldKey);
      }
    }

    container.appendChild(toast);

    // ── Track this toast ──
    const timer = setTimeout(() => dismissToast(toast, toastKey), duration);
    window.__toastMap.set(toastKey, { el: toast, timer });

    return toast;
  };

  function dismissToast(el, key) {
    if (!el || !el.parentNode) return;
    if (key && window.__toastMap.has(key)) {
      clearTimeout(window.__toastMap.get(key).timer);
      window.__toastMap.delete(key);
    }
    el.classList.add('out');
    setTimeout(() => { if (el.parentNode) el.remove(); }, 250);
  }

  function findToastKey(el) {
    for (const [key, val] of window.__toastMap) {
      if (val.el === el) return key;
    }
    return null;
  }

  window.showConfirm = function(message, title = "Xác nhận") {
    return new Promise(resolve => {
      const isDark = document.body.classList.contains("theme-dark");
      const bg  = isDark ? "#242526" : "#ffffff";
      const clr = isDark ? "#e4e6ea" : "#050505";
      const sub = isDark ? "#b0b3b8" : "#65676b";
      const bdr = isDark ? "#3e4042" : "#e4e6ea";
      const s2  = isDark ? "#3a3b3c" : "#f0f2f5";
      const overlay = document.createElement("div");
      overlay.style.cssText = `position:fixed;inset:0;z-index:10000;background:rgba(0,0,0,.55);display:flex;align-items:center;justify-content:center;padding:20px;backdrop-filter:blur(4px);animation:fadeIn .15s ease`;
      overlay.innerHTML = `
        <div style="background:${bg};border-radius:20px;padding:28px 28px 22px;max-width:380px;width:100%;
                    box-shadow:0 20px 60px rgba(0,0,0,.35);font-family:-apple-system,BlinkMacSystemFont,'Inter',sans-serif;
                    animation:slideUp .2s ease;">
          <div style="display:flex;align-items:center;gap:10px;margin-bottom:12px;">
            <span style="color:#f59e0b;"><svg viewBox="0 0 24 24" fill="currentColor" width="22" height="22"><path d="M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z"/></svg></span>
            <span style="font-size:17px;font-weight:700;color:${clr};">${title}</span>
          </div>
          <p style="font-size:14px;color:${sub};line-height:1.6;margin-bottom:24px;">${message}</p>
          <div style="display:flex;justify-content:flex-end;gap:8px;">
            <button id="c-no"  style="padding:10px 20px;border-radius:999px;font-size:14px;font-weight:600;background:${s2};color:${sub};border:1.5px solid ${bdr};cursor:pointer;">Hủy</button>
            <button id="c-yes" style="padding:10px 20px;border-radius:999px;font-size:14px;font-weight:700;background:linear-gradient(135deg,#e41e3f,#c01030);color:white;border:none;cursor:pointer;box-shadow:0 2px 10px rgba(228,30,63,.3);">Xác nhận</button>
          </div>
        </div>`;
      document.body.appendChild(overlay);
      overlay.querySelector("#c-no").onclick  = () => { overlay.remove(); resolve(false); };
      overlay.querySelector("#c-yes").onclick = () => { overlay.remove(); resolve(true);  };
      overlay.onclick = e => { if (e.target === overlay) { overlay.remove(); resolve(false); } };
    });
  };

  window.showPromptModal = function(message, placeholder = "", title = "Nhập thông tin") {
    return new Promise(resolve => {
      const isDark = document.body.classList.contains("theme-dark");
      const bg  = isDark ? "#242526" : "#ffffff";
      const clr = isDark ? "#e4e6ea" : "#050505";
      const sub = isDark ? "#b0b3b8" : "#65676b";
      const bdr = isDark ? "#3e4042" : "#e4e6ea";
      const s2  = isDark ? "#3a3b3c" : "#f0f2f5";
      const inp = isDark ? "#18191a" : "#f0f2f5";
      const overlay = document.createElement("div");
      overlay.style.cssText = `position:fixed;inset:0;z-index:10000;background:rgba(0,0,0,.55);display:flex;align-items:center;justify-content:center;padding:20px;backdrop-filter:blur(4px);animation:fadeIn .15s ease`;
      overlay.innerHTML = `
        <div style="background:${bg};border-radius:20px;padding:28px 28px 22px;max-width:420px;width:100%;
                    box-shadow:0 20px 60px rgba(0,0,0,.35);font-family:-apple-system,BlinkMacSystemFont,'Inter',sans-serif;
                    animation:slideUp .2s ease;">
          <div style="font-size:17px;font-weight:700;color:${clr};margin-bottom:8px;">${title}</div>
          <p style="font-size:14px;color:${sub};margin-bottom:14px;">${message}</p>
          <input id="p-input" type="text" placeholder="${placeholder}"
            style="width:100%;height:44px;padding:0 14px;background:${inp};border:2px solid ${bdr};
                   border-radius:12px;color:${clr};font-size:15px;outline:none;margin-bottom:18px;
                   transition:border-color .15s;">
          <div style="display:flex;justify-content:flex-end;gap:8px;">
            <button id="p-no"  style="padding:10px 20px;border-radius:999px;font-size:14px;font-weight:600;background:${s2};color:${sub};border:1.5px solid ${bdr};cursor:pointer;">Hủy</button>
            <button id="p-yes" style="padding:10px 20px;border-radius:999px;font-size:14px;font-weight:700;background:linear-gradient(135deg,#0084ff,#0066cc);color:white;border:none;cursor:pointer;box-shadow:0 2px 10px rgba(0,132,255,.3);">OK</button>
          </div>
        </div>`;
      document.body.appendChild(overlay);
      const input = overlay.querySelector("#p-input");
      input.focus();
      input.onfocus = () => input.style.borderColor = "#0084ff";
      input.onblur  = () => input.style.borderColor = bdr;
      const ok = () => { const v = input.value.trim(); overlay.remove(); resolve(v || null); };
      overlay.querySelector("#p-no").onclick  = () => { overlay.remove(); resolve(null); };
      overlay.querySelector("#p-yes").onclick = ok;
      input.addEventListener("keypress", e => { if (e.key === "Enter") ok(); });
      overlay.onclick = e => { if (e.target === overlay) { overlay.remove(); resolve(null); } };
    });
  };
})();

// ══════════════════════════════════════════
// AVATAR SYNC HELPERS
// ══════════════════════════════════════════
// Per-user avatar URL cache: { username: avatarUrl }
window._avatarCache = window._avatarCache || {};

function setUserAvatar(user, url) {
  if (!user) return;
  window._avatarCache[user] = url || null;
  syncAvatarsForUser(user, url);
}

function syncAvatarsForUser(user, url) {
  const initial = (user || "?")[0].toUpperCase();
  const imgHtml = url
    ? `<img src="${url}" style="width:100%;height:100%;object-fit:cover;border-radius:50%;" alt="${escapeHtml(user)}">`
    : null;

  // All [data-avatar-user=user] elements in DOM
  document.querySelectorAll(`[data-avatar-user="${CSS.escape(user)}"]`).forEach(av => {
    if (imgHtml) {
      av.innerHTML = imgHtml;
    } else {
      av.textContent = initial;
    }
  });
}

function syncAllAvatars() {
  // Update own avatar in cache first
  if (username && currentUserAvatar) {
    window._avatarCache[username] = currentUserAvatar;
  }

  // Col 1: nav avatar
  const navEl = document.getElementById("nav-avatar-el");
  if (navEl) {
    if (currentUserAvatar) {
      navEl.innerHTML = `<img src="${currentUserAvatar}" style="width:100%;height:100%;object-fit:cover;border-radius:50%;" alt="avatar">`;
    } else {
      navEl.textContent = (username || "?")[0].toUpperCase();
    }
  }

  // Col 3: all msg-avatar elements — sync by cached URL
  document.querySelectorAll(".msg-avatar[data-avatar-user]").forEach(av => {
    const user = av.dataset.avatarUser;
    const url  = window._avatarCache[user];
    if (url) {
      const existing = av.querySelector("img");
      if (existing) { existing.src = url; }
      else { av.innerHTML = `<img src="${url}" style="width:100%;height:100%;object-fit:cover;border-radius:50%;" alt="${escapeHtml(user)}"><`; }
    } else {
      if (!av.querySelector("img")) av.textContent = (user || "?")[0].toUpperCase();
    }
  });
}

// ── Sidebar ───────────────────────────────
function renderSidebar() {
  roomList.innerHTML = [...joinedRooms].map(([room, data]) => {
    const lastMsg    = data.msgs[data.msgs.length - 1];
    const isLocked   = !!data.locked;
    const hasPassword= !!data.passwordHash;
    const rawPreview = isLocked
      ? "🔒 Trò chuyện đã bị khóa"
      : lastMsg
        ? (lastMsg.content || (lastMsg.media ? "[file]" : ""))
        : "Chưa có tin nhắn";
    const preview    = rawPreview.length > 36 ? rawPreview.slice(0, 36) + "…" : rawPreview;
    const lockIcon   = isLocked ? '<span class="lock-badge" title="Đã khoá">🔒</span> ' : hasPassword ? '<span class="lock-badge" style="opacity:.5" title="Chat được bảo vệ">🔐</span> ' : "";
    const isActive   = room === currentRoom ? "active" : "";
    const timeStr    = lastMsg && lastMsg.timestamp ? formatMessageTime(lastMsg.timestamp) : "";
    const initial    = roomAvatar(room);
    
    // Trạng thái chưa đọc (unread)
    const unreadDot  = data.unread ? `<span class="room-unread-dot" title="Có tin nhắn mới"></span>` : "";
    const unreadClass= data.unread ? "unread" : "";

    return `
      <div class="room-item ${isActive} ${unreadClass}" onclick="_attemptSwitchRoom('${room}')">
        <div class="room-avatar">${initial}</div>
        <div class="room-info">
          <div class="room-name">${lockIcon}${escapeHtml(displayRoomName(room))}</div>
          <div class="room-msg">${escapeHtml(preview)}</div>
        </div>
        <div class="room-meta-right" style="display:flex;flex-direction:column;align-items:flex-end;gap:5px;flex-shrink:0;">
          ${timeStr ? `<div class="room-time">${timeStr}</div>` : ""}
          ${unreadDot}
        </div>
      </div>`;
  }).join("");

  updateChatNavBadge();
}

// ── Messages ──────────────────────────────
function renderMessages() {
  chatWindow.innerHTML = "";
  lastUser = "";
  if (!currentRoom) { clearLockOverlayState(); return; }
  const data = joinedRooms.get(currentRoom);
  if (!data)        { clearLockOverlayState(); return; }
  if (data.locked)  { renderLockOverlay(); return; }
  clearLockOverlayState();
  if (data.msgs) data.msgs.forEach(renderMessage);
}

function renderMessage(msg) {
  const isNewUser = lastUser !== msg.username;

  if (msg.msg_type === "system") {
    const div = document.createElement("div");
    div.className = "msg-group system";
    div.innerHTML = `<div class="msg">${escapeHtml(msg.content)}</div>`;
    chatWindow.appendChild(div);
  } else {
    const isOwn = msg.username === username;

    // ── User header (avatar + name) ──
    if (isNewUser) {
      const header = document.createElement("div");
      header.className = "msg-user-header" + (isOwn ? " own" : "");

      const avatarDiv = document.createElement("div");
      avatarDiv.className = "msg-avatar";
      avatarDiv.dataset.avatarUser = msg.username; // for sync

      const cachedAv = window._avatarCache?.[msg.username];
      if (cachedAv) {
        avatarDiv.innerHTML = `<img src="${cachedAv}" style="width:100%;height:100%;object-fit:cover;border-radius:50%;" alt="${escapeHtml(msg.username)}" onerror="this.parentElement.textContent='${escapeHtml((msg.username||'?')[0].toUpperCase())}';">`;
      } else {
        avatarDiv.textContent = (msg.username || "?")[0].toUpperCase();
      }

      const nameDiv = document.createElement("div");
      nameDiv.className = "msg-username";
      nameDiv.textContent = msg.username;

      header.appendChild(avatarDiv);
      header.appendChild(nameDiv);
      chatWindow.appendChild(header);
    }

    // ── Message row (actions + bubble) ──
    const row = document.createElement("div");
    row.className = "msg-group" + (isOwn ? " own" : "");

    // Bubble first (so flex order: bubble left when row-reversed)
    const msgEl = document.createElement("div");
    msgEl.className = "msg" + (msg.msg_type === "deleted" ? " deleted" : "");
    msgEl.appendChild(buildMessageContent(msg));

    if (msg.msg_type === "edited") {
      const meta = document.createElement("div");
      meta.className = "msg-meta";
      meta.textContent = `đã sửa${msg.timestamp ? " · " + formatMessageTime(msg.timestamp) : ""}`;
      msgEl.appendChild(meta);
    } else if (msg.timestamp) {
      const meta = document.createElement("div");
      meta.className = "msg-meta";
      meta.textContent = formatMessageTime(msg.timestamp);
      msgEl.appendChild(meta);
    }

    row.appendChild(msgEl);
    // Actions AFTER bubble in DOM; CSS order:-1 moves them visually LEFT
    if (isOwn && canModifyMessage(msg)) {
      const actions = document.createElement("div");
      actions.className = "msg-actions";
      actions.innerHTML = `
        <button class="msg-action msg-action-edit"  onclick="startEdit('${msg.message_id}')" title="Sửa">✏️</button>
        <button class="msg-action msg-action-delete" onclick="handleDeleteMessage('${msg.message_id}')" title="Xóa">🗑️</button>`;
      row.appendChild(actions);
    }
    chatWindow.appendChild(row);
  }

  lastUser = msg.username;
  chatWindow.scrollTop = chatWindow.scrollHeight;
}

async function handleDeleteMessage(messageId) {
  const ok = await showConfirm("Bạn có chắc muốn xóa tin nhắn này không?", "Xóa tin nhắn");
  if (ok) deleteMessage(messageId);
}

function refreshMessagesWithAvatar() {
  if (currentRoom && joinedRooms.has(currentRoom)) {
    const roomData = joinedRooms.get(currentRoom);
    if (roomData && roomData.msgs) {
      chatWindow.innerHTML = "";
      lastUser = "";
      roomData.msgs.forEach(msg => renderMessage(msg));
    }
  }
  // Also sync avatars in DOM without full re-render if possible
  syncAllAvatars();
}

// ── Nav active state helper ────────────────
const NAV_BTN_IDS = {
  chat:    "nav-btn-chat",
  friends: "nav-btn-friends",
  groups:  "nav-btn-groups",
  settings: null,
  admin:   null,
  profile: null,
};

function setNavActive(activeId) {
  document.querySelectorAll(".nav-btn").forEach(btn => {
    btn.classList.toggle("active", btn.id === activeId);
  });
}

// ── Show chat view (hide panels, activate Chat nav) ──
function showChatView() {
  closePanel();
  setNavActive("nav-btn-chat");
}

// ── Panel router ──────────────────────────
function openPanel(panel) {
  activePanel = panel;
  utilityPanel.classList.remove("hidden");
  utilityPanel.style.display = "flex";
  
  // Activate corresponding nav button
  const navId = NAV_BTN_IDS[panel];
  if (navId) setNavActive(navId);
  
  if (panel === "profile") {
    panelTitle.textContent = "Profile";
    panelBody.innerHTML    = "<div class='panel-section'><p style='color:var(--text-sub);font-size:13px;'>Đang tải profile...</p></div>";
    sendAction("profile_get");
  } else if (panel === "friends") {
    panelTitle.textContent = "Friends";
    renderFriendsPanel({ friends: [], incoming: [], outgoing: [] });
    sendAction("friends_list");
  } else if (panel === "groups") {
    panelTitle.textContent = "Groups";
    renderGroupsPanel({ groups: [], invites: [], join_requests: [] });
    sendAction("groups_list");
  } else if (panel === "admin") {
    panelTitle.textContent = "Admin";
    renderAdminPanel({ users: [] });
    sendAction("admin_users_list");
  } else {
    panelTitle.textContent = "Settings";
    renderSettingsPanel();
  }
}

function closePanel() {
  activePanel = "";
  utilityPanel.classList.add("hidden");
  panelBody.innerHTML = "";
}

function handleControlMessage(data) {
  const controlTypes = new Set(["profile_data", "friend_search_result", "friends_data", "groups_data", "admin_users_data"]);
  if (!controlTypes.has(data.msg_type)) return false;
  const payload = data.content ? JSON.parse(data.content) : null;

  if (data.msg_type === "profile_data") {
    const oldUsername = username;
    const usernameChanged = payload && payload.username && username !== payload.username;
    if (payload && payload.username && usernameChanged) username = payload.username;
    const oldAvatar = currentUserAvatar;
    if (payload && payload.avatar_url)   currentUserAvatar      = payload.avatar_url;
    if (payload && payload.display_name) currentUserDisplayName = payload.display_name;
    // Nếu username thay đổi (user login bằng email/phone), re-restore rooms và lock states
    if (usernameChanged) {
      const savedRooms = typeof getSavedRooms === "function" ? getSavedRooms() : [];
      savedRooms.forEach(id => {
        if (!joinedRooms.has(id) && typeof joinRoom === "function") joinRoom(id);
      });
      if (typeof restoreAllLockStates === "function") restoreAllLockStates();
    }
    // Jika username berubah (user login dengan email/phone), re-render messages agar alignment đúng
    if (usernameChanged) {
      if (currentRoom && !isRoomLocked(currentRoom)) {
        renderMessages();
      }
    }
    // Update avatar cache for own user
    if (username && currentUserAvatar) {
      window._avatarCache = window._avatarCache || {};
      window._avatarCache[username] = currentUserAvatar;
      syncAvatarsForUser(username, currentUserAvatar);
    }
    syncAllAvatars();
    if (oldAvatar !== currentUserAvatar) refreshMessagesWithAvatar();
    updateAdminVisibility(payload);
    if (activePanel === "profile") renderProfilePanel(payload);
  }
  if (data.msg_type === "friend_search_result") renderFriendSearchResult(payload);
  if (data.msg_type === "friends_data") {
    updateFriendsNavBadge(payload);
    renderFriendsPanel(payload);
  }
  if (data.msg_type === "groups_data") {
    if (payload && Array.isArray(payload)) {
      userGroups.clear();
      payload.forEach(g => { if (g.group_id) userGroups.set(g.group_id, g.group_name); });
    }
    // Auto-join group rooms so they appear in the sidebar after login
    if (payload && payload.groups && Array.isArray(payload.groups)) {
      payload.groups.forEach(g => {
        if (g.name) {
          const roomId = `group:${g.name}`;
          if (!joinedRooms.has(roomId)) {
            autoJoinGroupRoom(roomId);
          }
        }
      });
    }
    updateGroupsNavBadge(payload);
    renderGroupsPanel(payload);
    renderSidebar();
  }
  if (data.msg_type === "admin_users_data") renderAdminPanel(payload);
  return true;
}

// ── Profile panel ─────────────────────────
function renderProfilePanel(profile) {
  if (activePanel && activePanel !== "profile") return;
  activePanel = "profile";
  utilityPanel.classList.remove("hidden");
  utilityPanel.style.display = "flex";
  panelTitle.textContent = "Profile";
  updateAdminVisibility(profile);

  const avatarHtml = profile.avatar_url
    ? `<img src="${escapeAttr(profile.avatar_url)}" style="width:72px;height:72px;border-radius:50%;object-fit:cover;box-shadow:0 4px 12px rgba(88,101,242,.35);" alt="avatar">`
    : `<div style="width:72px;height:72px;background:var(--accent);border-radius:50%;display:flex;align-items:center;justify-content:center;color:white;font-size:28px;font-weight:700;">${escapeHtml((profile.username || "?")[0].toUpperCase())}</div>`;

  panelBody.innerHTML = `
    <div class="panel-section" style="align-items:center;text-align:center;">
      <h4>AVATAR</h4>
      <div style="margin:10px 0 14px;position:relative;display:inline-block;">
        ${avatarHtml}
      </div>
      <label class="upload-btn">
        <svg viewBox="0 0 24 24" fill="currentColor" width="14" height="14"><path d="M9 16h6v-6h4l-7-7-7 7h4zm-4 2h14v2H5z"/></svg>
        Chọn ảnh
        <input id="avatar-file" type="file" accept="image/*" onchange="uploadAvatar()" style="display:none;">
      </label>
      <p style="font-size:11px;color:var(--text-dim);">Tối đa 5MB · JPG, PNG, GIF</p>
    </div>
    <div class="panel-section">
      <h4>THÔNG TIN CÁ NHÂN</h4>
      <label>Tên đăng nhập</label>
      <input type="text" value="${escapeAttr(profile.username)}" disabled style="opacity:.55;">
      <label>Email</label>
      <input type="email" value="${escapeAttr(profile.email || "")}" disabled style="opacity:.55;">
      <label>Số điện thoại</label>
      <input type="tel" value="${escapeAttr(profile.phone || "")}" disabled style="opacity:.55;">
      <label>Tên hiển thị</label>
      <input id="profile-display" value="${escapeAttr(profile.display_name || profile.username)}" maxlength="100" placeholder="Tên hiển thị">
      <label>Giới thiệu</label>
      <textarea id="profile-bio" maxlength="500" placeholder="Giới thiệu ngắn">${escapeHtml(profile.bio || "")}</textarea>
      <button onclick="saveProfile()">Lưu thay đổi</button>
    </div>`;
}

function uploadAvatar() {
  const fileInput = document.getElementById("avatar-file");
  const file = fileInput.files[0];
  if (!file) return;
  if (file.size > 5 * 1024 * 1024) {
    showToast("Kích thước file quá lớn (tối đa 5MB)", "error");
    return;
  }
  const reader = new FileReader();
  reader.onload = function(e) {
    const displayName = document.getElementById("profile-display")?.value.trim() || "";
    const bio         = document.getElementById("profile-bio")?.value.trim() || "";
    // Update local state and avatar cache immediately
    currentUserAvatar = e.target.result;
    window._avatarCache = window._avatarCache || {};
    window._avatarCache[username] = e.target.result;
    syncAllAvatars();
    syncAvatarsForUser(username, e.target.result);
    refreshMessagesWithAvatar();
    // Re-render profile panel preview
    const imgEl = panelBody.querySelector("img[alt='avatar']");
    if (imgEl) imgEl.src = e.target.result;
    // Send to server
    sendAction("profile_update", { content: JSON.stringify({ display_name: displayName, bio, avatar_url: e.target.result }) });
    showToast("Avatar đã được cập nhật!", "success");
  };
  reader.readAsDataURL(file);
}

function saveProfile() {
  const displayName = document.getElementById("profile-display")?.value.trim() || "";
  const bio         = document.getElementById("profile-bio")?.value.trim() || "";
  sendAction("profile_update", { content: JSON.stringify({ display_name: displayName, bio }) });
  showToast("Thông tin cá nhân đã được cập nhật!", "success");
}

function updateAdminVisibility(profile) {
  currentUserRole = profile?.role || "";
  adminPanelBtn.classList.toggle("hidden", currentUserRole !== "admin");
}

// ── Friends panel ─────────────────────────
function renderFriendsPanel(data) {
  if (activePanel && activePanel !== "friends") return;
  activePanel = "friends";
  utilityPanel.classList.remove("hidden");
  utilityPanel.style.display = "flex";
  panelTitle.textContent = "Friends";
  panelBody.innerHTML = `
    <div class="panel-section">
      <h4>TÌM KIẾM</h4>
      <input id="friend-search" placeholder="Nhập username...">
      <button onclick="searchFriend()">Tìm kiếm</button>
      <div id="friend-search-result"></div>
    </div>
    <div class="panel-section"><h4>BẠN BÈ</h4>${renderUserRows(data.friends || [], u => `<button onclick="joinRoom(privateRoomId(username, '${escapeJs(u.username)}'))">Nhắn tin</button>`)}</div>
    <div class="panel-section"><h4>LỜI MỜI ĐẾN</h4>${renderUserRows(data.incoming || [], u => `<button onclick="friendRespond('${escapeJs(u.username)}', true)">Chấp nhận</button><button onclick="friendRespond('${escapeJs(u.username)}', false)" style="background:var(--surface2);color:var(--text-sub);border:1px solid var(--border);">Từ chối</button>`)}</div>
    <div class="panel-section"><h4>LỜI MỜI ĐI</h4>${renderUserRows(data.outgoing || [], () => `<span class="list-note" style="font-size:12px;padding:3px 8px;background:var(--surface2);border-radius:6px;">Đang chờ</span>`)}</div>`;
}

function searchFriend() {
  const target = document.getElementById("friend-search").value.trim();
  if (target) sendAction("friend_search", { target });
}

function renderFriendSearchResult(user) {
  const box = document.getElementById("friend-search-result");
  if (!box) return;
  if (!user) { box.innerHTML = `<div class="list-note" style="margin-top:8px;">Không tìm thấy người dùng</div>`; return; }
  box.innerHTML = `<div class="list-row" style="margin-top:8px;"><div><div class="list-title">${escapeHtml(user.display_name || user.username)}</div><div class="list-note">@${escapeHtml(user.username)}</div></div><button onclick="sendFriendRequest('${escapeJs(user.username)}')">Kết bạn</button></div>`;
}

function sendFriendRequest(target) {
  sendAction("friend_request", { target });
  showToast(`Đã gửi lời mời kết bạn tới ${target}`, "success");
}
function friendRespond(target, accept) {
  sendAction(accept ? "friend_accept" : "friend_decline", { target });
  showToast(accept ? `Đã chấp nhận lời mời của ${target}` : `Đã từ chối lời mời của ${target}`, accept ? "success" : "info");
}

// ── Groups panel ──────────────────────────
function renderGroupsPanel(data) {
  if (activePanel && activePanel !== "groups") return;
  activePanel = "groups";
  utilityPanel.classList.remove("hidden");
  utilityPanel.style.display = "flex";
  panelTitle.textContent = "Groups";
  panelBody.innerHTML = `
    <div class="panel-section">
      <h4>CREATE / JOIN</h4>
      <input id="group-name" placeholder="group name">
      <button onclick="createGroup()">Create group</button>
      <button onclick="requestJoinGroup()" style="background:var(--surface2);color:var(--text-sub);border:1px solid var(--border);">Request to join</button>
    </div>
    <div class="panel-section"><h4>YOUR GROUPS</h4>${renderGroupRows(data.groups || [])}</div>
    <div class="panel-section"><h4>INVITES</h4>${(data.invites || []).map(g => `<div class="list-row"><div><div class="list-title">${escapeHtml(g.name)}</div><div class="list-note">Owner: ${escapeHtml(g.owner)}</div></div><button onclick="acceptGroupInvite('${escapeJs(g.name)}')">Accept</button></div>`).join("") || "<div class='list-note'>No invites</div>"}</div>
    <div class="panel-section"><h4>JOIN REQUESTS</h4>${(data.join_requests || []).map(r => `<div class="list-row"><div><div class="list-title">${escapeHtml(r.username)}</div><div class="list-note">${escapeHtml(r.group)}</div></div><div style="display:flex;gap:6px;"><button onclick="respondGroupJoin('${escapeJs(r.group)}','${escapeJs(r.username)}',true)">Accept</button><button onclick="respondGroupJoin('${escapeJs(r.group)}','${escapeJs(r.username)}',false)" style="background:var(--surface2);color:var(--text-sub);border:1px solid var(--border);">Decline</button></div></div>`).join("") || "<div class='list-note'>No requests</div>"}</div>`;
}

function renderGroupRows(groups) {
  if (!groups.length) return "<div class='list-note'>No groups</div>";
  return groups.map(g => `<div class="list-row"><div><div class="list-title">${escapeHtml(g.name)}</div><div class="list-note">${escapeHtml(g.role)} · owner ${escapeHtml(g.owner)}</div></div><div style="display:flex;gap:6px;"><button onclick="joinRoom('group:${escapeJs(g.name)}')">Open</button>${g.role === "owner" ? `<button onclick="inviteToGroup('${escapeJs(g.name)}')" style="background:var(--surface2);color:var(--text-sub);border:1px solid var(--border);">Invite</button>` : ""}</div></div>`).join("");
}

function createGroup()      { const name = document.getElementById("group-name").value.trim(); if (name) { sendAction("group_create", { target: name }); showToast(`Đã tạo nhóm "${name}"`, "success"); } }
function requestJoinGroup() { const name = document.getElementById("group-name").value.trim(); if (name) { sendAction("group_join_request", { target: name }); showToast(`Đã gửi yêu cầu tham gia nhóm "${name}"`, "info"); } }

async function inviteToGroup(group) {
  const target = await showPromptModal("Nhập username người muốn mời:", "username", `Mời vào nhóm ${group}`);
  if (target) {
    sendAction("group_invite", { room: `group:${group}`, target });
    showToast(`Đã gửi lời mời tới ${target}`, "success");
  }
}

function acceptGroupInvite(group) {
  sendAction("group_invite_accept", { target: group });
  showToast(`Đã tham gia nhóm "${group}"`, "success");
  setTimeout(() => joinRoom(`group:${group}`), 500);
}
function respondGroupJoin(group, target, accept) {
  sendAction(accept ? "group_join_accept" : "group_join_decline", { room: `group:${group}`, target });
  showToast(accept ? `Đã chấp nhận ${target} vào nhóm` : `Đã từ chối yêu cầu của ${target}`, accept ? "success" : "info");
}

// ── Settings panel ────────────────────────
function renderSettingsPanel() {
  const theme = localStorage.getItem("chat-theme") || "light";
  const sound = localStorage.getItem("chat-sound") || "off";
  panelBody.innerHTML = `
    <!-- Section: Giao diện -->
    <div class="panel-section settings-section">
      <div class="settings-section-header">
        <svg viewBox="0 0 24 24" fill="currentColor" width="16" height="16" style="flex-shrink:0;opacity:.7;"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm-.5-13v4h-2v2h2v4h2v-4h2v-2h-2V7h-2z"/></svg>
        <span>Giao diện</span>
      </div>
      <label>Chủ đề màu sắc</label>
      <div style="display:flex;gap:8px;margin-top:2px;">
        <button onclick="setTheme('light')" class="theme-toggle-btn ${theme === 'light' ? 'active' : ''}">☀️ Light</button>
        <button onclick="setTheme('dark')"  class="theme-toggle-btn ${theme === 'dark'  ? 'active' : ''}">🌙 Dark</button>
      </div>
      <label style="margin-top:8px;">Âm thanh thông báo</label>
      <select id="setting-sound" onchange="saveUiSettings()">
        <option value="off" ${sound === "off" ? "selected" : ""}>Tắt âm thanh</option>
        <option value="on"  ${sound === "on"  ? "selected" : ""}>Bật âm thanh</option>
      </select>
    </div>

    <hr class="settings-divider">

    <!-- Section: Đổi mật khẩu -->
    <div class="panel-section settings-section">
      <div class="settings-section-header">
        <svg viewBox="0 0 24 24" fill="currentColor" width="16" height="16" style="flex-shrink:0;opacity:.7;"><path d="M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zm-6 9c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm3.1-9H8.9V6c0-1.71 1.39-3.1 3.1-3.1 1.71 0 3.1 1.39 3.1 3.1v2z"/></svg>
        <span>Đổi mật khẩu</span>
      </div>
      <input id="old-password" type="password" placeholder="Mật khẩu hiện tại">
      <input id="new-password" type="password" placeholder="Mật khẩu mới">
      <button onclick="changePassword()" class="settings-btn-password">Cập nhật mật khẩu</button>
    </div>

    <hr class="settings-divider">

    <!-- Section: Thông tin ứng dụng -->
    <div class="panel-section settings-section" style="opacity:.7;">
      <div class="settings-section-header">
        <svg viewBox="0 0 24 24" fill="currentColor" width="16" height="16" style="flex-shrink:0;opacity:.7;"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/></svg>
        <span>Thông tin</span>
      </div>
      <div style="font-size:13px;color:var(--text-sub);line-height:1.6;">
        <div style="display:flex;justify-content:space-between;"><span>Phiên bản</span><span style="font-weight:500;">1.0.0</span></div>
        <div style="display:flex;justify-content:space-between;margin-top:4px;"><span>Nền tảng</span><span style="font-weight:500;">WebSocket + Rust</span></div>
      </div>
    </div>`;
}

function setTheme(theme) {
  localStorage.setItem("chat-theme", theme);
  applySavedSettings();
  renderSettingsPanel();
  showToast(`Đã chuyển sang chế độ ${theme === "dark" ? "tối 🌙" : "sáng ☀️"}`, "success", 2500);
}

// ── Admin panel ───────────────────────────
function renderAdminPanel(data) {
  if (activePanel && activePanel !== "admin") return;
  activePanel = "admin";
  utilityPanel.classList.remove("hidden");
  utilityPanel.style.display = "flex";
  panelTitle.textContent = "Admin";
  panelBody.innerHTML = `
    <div class="panel-section">
      <h4>QUẢN LÝ NGƯỜI DÙNG</h4>
      <input id="admin-user-search" placeholder="Tìm username hoặc tên hiển thị" onkeypress="if(event.key==='Enter') searchAdminUsers()">
      <button onclick="searchAdminUsers()">Tìm kiếm</button>
    </div>
    <div class="panel-section">
      <h4>DANH SÁCH</h4>
      ${renderAdminUserRows(data.users || [])}
    </div>`;
}

function renderAdminUserRows(users) {
  if (!users.length) return "<div class='list-note'>Không có người dùng</div>";
  return users.map(user => `
    <div class="list-row" style="flex-wrap:wrap;gap:8px;">
      <div style="flex:1;min-width:0;">
        <div class="list-title">${escapeHtml(user.display_name || user.username)}</div>
        <div class="list-note">@${escapeHtml(user.username)} · ${user.is_active ? "✅ active" : "❌ inactive"}</div>
        <div class="list-note">${escapeHtml(formatAdminDate(user.created_at))}</div>
      </div>
      <div style="display:flex;flex-direction:column;gap:4px;align-items:flex-end;">
        <select id="admin-role-${user.id}" style="font-size:12px;padding:3px 6px;border-radius:6px;background:var(--surface2);color:var(--text);border:1px solid var(--border);">
          <option value="user"  ${user.role === "user"  ? "selected" : ""}>user</option>
          <option value="admin" ${user.role === "admin" ? "selected" : ""}>admin</option>
        </select>
        <label style="display:flex;align-items:center;gap:4px;font-size:12px;color:var(--text-sub);cursor:pointer;">
          <input id="admin-active-${user.id}" type="checkbox" ${user.is_active ? "checked" : ""}> Active
        </label>
        <button onclick="saveAdminUser(${user.id})" style="font-size:12px;padding:4px 10px;">Lưu</button>
      </div>
    </div>`).join("");
}

function searchAdminUsers() {
  const target = document.getElementById("admin-user-search")?.value.trim() || "";
  sendAction("admin_users_list", { target });
}

function saveAdminUser(userId) {
  const role     = document.getElementById(`admin-role-${userId}`)?.value;
  const isActive = document.getElementById(`admin-active-${userId}`)?.checked;
  sendAction("admin_user_update", { content: JSON.stringify({ user_id: userId, role, is_active: isActive }) });
  showToast("Đã cập nhật thông tin người dùng", "success");
}

// ── Shared row renderers ───────────────────
function renderUserRows(users, actionRenderer) {
  if (!users.length) return "<div class='list-note' style='font-size:13px;color:var(--text-dim);'>Trống</div>";
  return users.map(u => `
    <div class="list-row">
      <div><div class="list-title">${escapeHtml(u.display_name || u.username)}</div><div class="list-note">@${escapeHtml(u.username)}</div></div>
      <div style="display:flex;gap:6px;">${actionRenderer(u)}</div>
    </div>`).join("");
}

// ── Badge Helpers ───────────────────────────
function updateChatNavBadge() {
  const hasUnread = [...joinedRooms.values()].some(data => data.unread);
  const chatBtn = document.getElementById("nav-btn-chat");
  if (!chatBtn) return;
  
  let badge = chatBtn.querySelector(".nav-badge-dot");
  if (hasUnread) {
    if (!badge) {
      badge = document.createElement("span");
      badge.className = "nav-badge-dot";
      chatBtn.appendChild(badge);
    }
  } else {
    badge?.remove();
  }
}

function updateFriendsNavBadge(payload) {
  const friendsBtn = document.getElementById("nav-btn-friends");
  if (!friendsBtn) return;
  const incomingCount = payload && Array.isArray(payload.incoming) ? payload.incoming.length : 0;
  let badge = friendsBtn.querySelector(".nav-badge");
  if (incomingCount > 0) {
    if (!badge) {
      badge = document.createElement("span");
      badge.className = "nav-badge";
      friendsBtn.appendChild(badge);
    }
    badge.textContent = incomingCount;
    badge.classList.remove("hidden");
  } else {
    badge?.remove();
  }
}

function updateGroupsNavBadge(payload) {
  const groupsBtn = document.getElementById("nav-btn-groups");
  if (!groupsBtn) return;
  
  // Calculate total group requests and invites
  const invitesCount = payload && Array.isArray(payload.invites) ? payload.invites.length : 0;
  const requestsCount = payload && Array.isArray(payload.join_requests) ? payload.join_requests.length : 0;
  const totalCount = invitesCount + requestsCount;
  
  let badge = groupsBtn.querySelector(".nav-badge");
  if (totalCount > 0) {
    if (!badge) {
      badge = document.createElement("span");
      badge.className = "nav-badge";
      groupsBtn.appendChild(badge);
    }
    badge.textContent = totalCount;
    badge.classList.remove("hidden");
  } else {
    badge?.remove();
  }
}
