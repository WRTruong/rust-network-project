// ─────────────────────────────────────────
// chat.js — Rooms, lock, send, render
// ─────────────────────────────────────────

// ── Room helpers ──────────────────────────
function privateRoomId(left, right) {
  const participants = [String(left || "").trim(), String(right || "").trim()].sort();
  return `dm:${participants[0]}:${participants[1]}`;
}

function isPrivateRoom(room) { return room && room.startsWith("dm:"); }
function isGroupRoom(room)   { return room && room.startsWith("group:"); }

function getPrivateTarget(room) {
  if (!isPrivateRoom(room)) return "";
  const participants = room.replace("dm:", "").split(":");
  return participants.find(name => name !== username) || username;
}

function displayRoomName(room) {
  if (isPrivateRoom(room)) return `@${getPrivateTarget(room)}`;
  if (isGroupRoom(room))   return `# ${room.replace("group:", "")}`;
  return room;
}

function roomAvatar(room) {
  const label = displayRoomName(room);
  return label[0] ? label[0].toUpperCase() : "?";
}

// ── Room persistence helpers ──────────────
function saveRoomToLocalStorage(room) {
  if (!username) return;
  try {
    const key = `u:${username}:rooms`;
    const saved = JSON.parse(localStorage.getItem(key) || "[]");
    if (!saved.includes(room)) {
      saved.push(room);
      localStorage.setItem(key, JSON.stringify(saved));
    }
  } catch(e) {}
}

function removeRoomFromLocalStorage(room) {
  if (!username) return;
  try {
    const key = `u:${username}:rooms`;
    const saved = JSON.parse(localStorage.getItem(key) || "[]");
    const filtered = saved.filter(r => r !== room);
    localStorage.setItem(key, JSON.stringify(filtered));
  } catch(e) {}
}

function getSavedRooms() {
  if (!username) return [];
  try {
    // Đọc từ key mới (mảng các room)
    const raw = localStorage.getItem(`u:${username}:rooms`);
    if (raw) return JSON.parse(raw);
    
    // Migration: đọc key cũ (u:USERNAME:group_*) và trả về
    const prefix = `u:${username}:group_`;
    const oldRooms = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key && key.startsWith(prefix)) {
        try {
          const data = JSON.parse(localStorage.getItem(key));
          if (data && data.id) {
            oldRooms.push(data.id);
            localStorage.removeItem(key); // migrate
          }
        } catch(e) {}
      }
    }
    // Lưu vào key mới và xóa key cũ
    if (oldRooms.length > 0) {
      localStorage.setItem(`u:${username}:rooms`, JSON.stringify(oldRooms));
    }
    return oldRooms;
  } catch(e) { return []; }
}

// ── Auto-join a room without switching (used after login) ──
function autoJoinGroupRoom(room) {
  if (joinedRooms.has(room)) return;
  const savedLock = loadRoomLockState(room, typeof username !== "undefined" ? username : "");
  joinedRooms.set(room, { msgs: [], locked: savedLock.locked, passwordHash: savedLock.passwordHash });
  saveRoomToLocalStorage(room);
  if (ws && ws.readyState === WebSocket.OPEN) {
    ws.send(JSON.stringify({
      msg_type: "join",
      username, password: "", room, content: "", target: "", users: []
    }));
  }
}

// ── Room join / switch / leave ─────────────
function joinRoom(room) {
  if (joinedRooms.has(room)) {
    // Re-apply persistent lock state cho user hiện tại
    const savedLock = loadRoomLockState(room, typeof username !== "undefined" ? username : "");
    const existing  = joinedRooms.get(room);
    if (savedLock.passwordHash && !existing.passwordHash) {
      existing.locked       = savedLock.locked;
      existing.passwordHash = savedLock.passwordHash;
      joinedRooms.set(room, existing);
    }
    _attemptSwitchRoom(room);
    return;
  }
  // Load lock state theo username hiện tại (mỗi user có lock riêng)
  const savedLock = loadRoomLockState(room, typeof username !== "undefined" ? username : "");
  joinedRooms.set(room, { msgs: [], locked: savedLock.locked, passwordHash: savedLock.passwordHash });

  saveRoomToLocalStorage(room);

  ws.send(JSON.stringify({
    msg_type: "join",
    username, password: "", room, content: "", target: "", users: []
  }));
  _attemptSwitchRoom(room);
}

function _attemptSwitchRoom(room) {
  const roomData = joinedRooms.get(room);
  if (roomData && roomData.locked) {
    // If the room is locked, don't just switch. Show unlock modal.
    currentRoom = room; // Temporary set to allow modal to unlock it
    showLockModal("unlock");
    return;
  }
  switchRoom(room);
}

function switchRoom(room) {
  currentRoom = room;
  lastUser    = "";
  cancelEdit();

  if (!room) {
    roomTitle.textContent = "Select a room";
    renderSidebar();
    renderMessages();
    return;
  }

  const roomData = joinedRooms.get(room);
  if (roomData) roomData.unread = false;

  const dname = displayRoomName(room);
  roomTitle.textContent = dname;
  const peerAv = document.getElementById("chat-peer-avatar");
  if (peerAv) peerAv.textContent = roomAvatar(room);
  const peerStatus = document.getElementById("chat-peer-status");
  if (peerStatus) peerStatus.textContent = isPrivateRoom(room) ? "Đang hoạt động" : "Nhóm";

  renderSidebar();
  updateLockActions();
  renderMessages();
}

function handleLeave() {
  if (!currentRoom) { showToast("Không có phòng nào để rời", "warning"); return; }
  ws.send(JSON.stringify({
    msg_type: "leave", username, password: "", room: currentRoom, content: "", target: "", users: []
  }));
  
  // Xóa khỏi localStorage cách ly của user hiện tại
  removeRoomFromLocalStorage(currentRoom);
  localStorage.removeItem(`group_${currentRoom}`);

  joinedRooms.delete(currentRoom);
  currentRoom = null;
  roomTitle.textContent = "Select a room";
  lastUser = "";
  renderSidebar();
  renderMessages();
}

async function createRoomChat() {
  const name = await showPromptModal("Nhập tên phòng chat:", "tên phòng", "Tạo phòng mới");
  if (name) joinRoom(name.trim());
}

async function createPrivateChat() {
  const target = await showPromptModal("Nhập username người muốn nhắn tin:", "username", "Tin nhắn riêng");
  if (target && target.trim() !== username) {
    joinRoom(privateRoomId(username, target.trim()));
  }
}

// ── Lock ──────────────────────────────────
function normalizeLockKey(room, forUser) {
  // Key PHẢI có username để mỗi user có lock state riêng
  // forUser: username cụ thể (dùng khi lưu/đọc) — nếu không có thì dùng global username
  const user = forUser || (typeof username !== "undefined" ? username : "");
  const roomNorm = String(room || "").trim().toLowerCase();
  if (user) return `chat_lock_u:${user}:${roomNorm}`;
  // Fallback khi username chưa set — key tạm thời, sẽ được migrate sau
  return `chat_lock_${roomNorm}`;
}

function _lockKeyPrefix(forUser) {
  const user = forUser || (typeof username !== "undefined" ? username : "");
  return user ? `chat_lock_u:${user}:` : `chat_lock_`;
}

function loadRoomLockState(room, forUser) {
  try {
    // Thử key mới (có username) trước
    const newKey = normalizeLockKey(room, forUser);
    const raw    = localStorage.getItem(newKey);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        locked:       !!parsed.locked,
        passwordHash: typeof parsed.passwordHash === "string" ? parsed.passwordHash : ""
      };
    }
    // Migration: thử key cũ (không có username)
    const oldKey = `chat_lock_${String(room || "").trim().toLowerCase()}`;
    const oldRaw = localStorage.getItem(oldKey);
    if (oldRaw) {
      const parsed = JSON.parse(oldRaw);
      const state  = {
        locked:       !!parsed.locked,
        passwordHash: typeof parsed.passwordHash === "string" ? parsed.passwordHash : ""
      };
      // Migrate sang key mới và xóa key cũ
      if (state.passwordHash && forUser) {
        localStorage.setItem(newKey, JSON.stringify(state));
        localStorage.removeItem(oldKey);
      }
      return state;
    }
    return { locked: false, passwordHash: "" };
  } catch { return { locked: false, passwordHash: "" }; }
}

function saveRoomLockState(room, state) {
  // Lưu theo user hiện tại — KHÔNG ảnh hưởng user khác
  const key    = normalizeLockKey(room);
  // Xóa key cũ (migration cleanup)
  const oldKey = `chat_lock_${String(room || "").trim().toLowerCase()}`;
  if (key !== oldKey) localStorage.removeItem(oldKey);

  if (!state.passwordHash && !state.locked) {
    localStorage.removeItem(key);
    return;
  }
  localStorage.setItem(key, JSON.stringify({
    locked:       !!state.locked,
    passwordHash: state.passwordHash
  }));
}

function getRoomLockData(room) {
  const data = joinedRooms.get(room);
  return data ? { locked: !!data.locked, passwordHash: data.passwordHash || "" } : { locked: false, passwordHash: "" };
}

function setRoomLockState(room, { locked, passwordHash }) {
  const roomData = joinedRooms.get(room);
  if (!roomData) return;
  roomData.locked = locked;
  roomData.passwordHash = passwordHash;
  joinedRooms.set(room, roomData);
  saveRoomLockState(room, { locked, passwordHash });
  updateLockActions();
  // Re-render sidebar so lock icon updates immediately
  if (typeof renderSidebar === "function") renderSidebar();
}

async function hashPassword(password) {
  const data = new TextEncoder().encode(password);
  const hashBuffer = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(hashBuffer)).map(b => b.toString(16).padStart(2, "0")).join("");
}

function getActiveRoomData()  { return currentRoom ? joinedRooms.get(currentRoom) : null; }
function canRoomBeSent()      { const d = getActiveRoomData(); return !d || !d.locked; }
function isRoomLocked(room)   { const d = joinedRooms.get(room); return d ? !!d.locked : false; }

function updateLockActions() {
  const roomData = getActiveRoomData();
  const isLocked = roomData && roomData.locked;
  const hasPwd   = roomData && !!roomData.passwordHash;

  // Update header lock button icon & tooltip
  const lockSvgLocked   = `<svg viewBox="0 0 24 24" fill="currentColor" width="18" height="18"><path d="M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zm-6 9c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm3.1-9H8.9V6c0-1.71 1.39-3.1 3.1-3.1 1.71 0 3.1 1.39 3.1 3.1v2z"/></svg>`;
  const lockSvgUnlocked = `<svg viewBox="0 0 24 24" fill="currentColor" width="18" height="18"><path d="M12 1C8.676 1 6 3.676 6 7v1H4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V10a2 2 0 0 0-2-2H10V7c0-1.103.897-2 2-2s2 .897 2 2v1h2V7c0-3.324-2.676-6-6-6zm0 12a2 2 0 1 1 .001 4.001A2 2 0 0 1 12 13z"/></svg>`;

  if (!roomData) {
    btnLock.innerHTML = lockSvgLocked;
    btnLock.title = "Khoá chat";
    btnLock.classList.remove("hidden");
    btnChangeLock.classList.add("hidden");
    return;
  }
  if (!hasPwd) {
    btnLock.innerHTML = lockSvgLocked;
    btnLock.title = "Thiết lập khoá chat";
    btnLock.style.color = "";
    btnChangeLock.classList.add("hidden");
  } else if (isLocked) {
    btnLock.innerHTML = lockSvgUnlocked;
    btnLock.title = "Chat đang khoá — nhấn để mở";
    btnLock.style.color = "var(--warn, #f59e0b)";
    btnChangeLock.classList.remove("hidden");
  } else {
    btnLock.innerHTML = lockSvgLocked;
    btnLock.title = "Bật khoá chat";
    btnLock.style.color = "";
    btnChangeLock.classList.remove("hidden");
  }
}

function toggleRoomLock() {
  if (!currentRoom) return;
  const roomData = getActiveRoomData();
  if (!roomData || !roomData.passwordHash) { showLockModal("create");  return; }
  if (roomData.locked)                     { showLockModal("unlock");  return; }
  showLockModal("enable");
}

function showLockModal(mode) {
  if (!currentRoom) return;
  lockModalMode = mode;
  lockCurrentPassword.value = "";
  lockNewPassword.value     = "";
  lockConfirmPassword.value = "";
  lockModalError.classList.add("hidden");
  lockCurrentRow.classList.add("hidden");
  lockNewRow.classList.add("hidden");
  lockConfirmRow.classList.add("hidden");

  if (mode === "create")  { lockModalTitle.textContent = "Tạo mật khẩu khóa chat";  lockNewRow.classList.remove("hidden"); lockConfirmRow.classList.remove("hidden"); }
  if (mode === "unlock")  { lockModalTitle.textContent = "Mở khóa chat";             lockCurrentRow.classList.remove("hidden"); }
  if (mode === "change")  { lockModalTitle.textContent = "Đổi mật khẩu khóa chat";  lockCurrentRow.classList.remove("hidden"); lockNewRow.classList.remove("hidden"); lockConfirmRow.classList.remove("hidden"); }
  if (mode === "enable")  { lockModalTitle.textContent = "Bật khóa chat";            lockCurrentRow.classList.remove("hidden"); }

  lockModal.classList.remove("hidden");
  lockCurrentPassword.focus();
}

function closeLockModal() { lockModal.classList.add("hidden"); }

function setLockModalError(message) {
  lockModalError.textContent = message;
  lockModalError.classList.remove("hidden");
}

async function submitLockModal() {
  const roomData = getActiveRoomData();
  if (!currentRoom || !roomData) { setLockModalError("Vui lòng chọn phòng trước."); return; }

  const current = lockCurrentPassword.value.trim();
  const next    = lockNewPassword.value.trim();
  const confirm = lockConfirmPassword.value.trim();

  if (lockModalMode === "create") {
    if (!next || !confirm) { setLockModalError("Vui lòng nhập mật khẩu và xác nhận."); return; }
    if (next !== confirm)  { setLockModalError("Mật khẩu và xác nhận không trùng khớp."); return; }
    const hash = await hashPassword(next);
    setRoomLockState(currentRoom, { locked: true, passwordHash: hash });
    closeLockModal(); renderMessages(); return;
  }

  if (lockModalMode === "unlock") {
    if (!current) { setLockModalError("Vui lòng nhập mật khẩu hiện tại."); return; }
    const hash = await hashPassword(current);
    if (hash !== roomData.passwordHash) { setLockModalError("Mật khẩu không đúng."); return; }
    setRoomLockState(currentRoom, { locked: false, passwordHash: roomData.passwordHash });
    closeLockModal(); renderMessages(); return;
  }

  if (lockModalMode === "enable") {
    if (!current) { setLockModalError("Vui lòng nhập mật khẩu khóa hiện tại."); return; }
    const hash = await hashPassword(current);
    if (hash !== roomData.passwordHash) { setLockModalError("Mật khẩu không đúng."); return; }
    setRoomLockState(currentRoom, { locked: true, passwordHash: roomData.passwordHash });
    closeLockModal(); renderMessages(); return;
  }

  if (lockModalMode === "change") {
    if (!current || !next || !confirm) { setLockModalError("Vui lòng điền đầy đủ cả 3 trường."); return; }
    if (next !== confirm) { setLockModalError("Mật khẩu mới và xác nhận không trùng khớp."); return; }
    const currentHash = await hashPassword(current);
    if (currentHash !== roomData.passwordHash) { setLockModalError("Mật khẩu hiện tại không đúng."); return; }
    const newHash = await hashPassword(next);
    setRoomLockState(currentRoom, { locked: roomData.locked, passwordHash: newHash });
    closeLockModal(); return;
  }
}

function renderLockOverlay() {
  const roomData = getActiveRoomData();
  if (!roomData || !roomData.locked) return false;
  chatWindow.innerHTML = `
    <div class="locked-overlay">
      <div class="locked-overlay-card">
        <div class="lock-icon">🔒</div>
        <h4>Cuộc trò chuyện đã được khoá</h4>
        <p>Vui lòng nhập mật khẩu để xem nội dung và gửi tin nhắn.</p>
        <button onclick="showLockModal('unlock')">Mở khoá ngay</button>
      </div>
    </div>`;
  messageInput.disabled = true;
  messageInput.placeholder = "🔒 Chat đang bị khoá...";
  return true;
}

function clearLockOverlayState() {
  messageInput.disabled = false;
  messageInput.placeholder = "Nhập tin nhắn...";
}

// ── Messages ──────────────────────────────
function sendMessage() {
  const input   = document.getElementById("message-input");
  const content = input.value.trim();
  if (!ws || ws.readyState !== WebSocket.OPEN) return;

  if (editingMessageId) {
    if (!content) return;
    ws.send(JSON.stringify({
      msg_type: "edit", username, password: "", content,
      room: currentRoom, target: "", users: [], message_id: editingMessageId
    }));
    cancelEdit();
    return;
  }

  if (mediaFile.files.length > 0) { sendMedia(content); return; }
  if (!content) return;
  if (!currentRoom) { showToast("Vui lòng chọn phòng trước khi gửi tin nhắn", "warning"); return; }
  if (!canRoomBeSent()) { showToast("Chat đang bị khóa. Vui lòng mở khóa trước.", "warning"); return; }

  const target = isPrivateRoom(currentRoom) ? getPrivateTarget(currentRoom) : "";
  ws.send(JSON.stringify({
    msg_type: "message", username, password: "", content,
    room: currentRoom, target, users: [],
    avatar_url: typeof currentUserAvatar !== "undefined" ? currentUserAvatar : ""
  }));
  input.value = "";
}

function startEdit(messageId) {
  const msg = findMessage(messageId);
  if (!msg) return;
  editingMessageId = messageId;
  mediaPanel.classList.add("hidden");
  editLabel.textContent = `Dang sua: ${msg.content.substring(0, 60)}`;
  editBar.classList.remove("hidden");
  messageInput.value = msg.content;
  messageInput.focus();
}

function cancelEdit() {
  editingMessageId = "";
  editBar.classList.add("hidden");
  messageInput.value = "";
}

function deleteMessage(messageId) {
  // confirm handled by handleDeleteMessage
  ws.send(JSON.stringify({
    msg_type: "delete", username, password: "", content: "",
    room: currentRoom, target: "", users: [], message_id: messageId
  }));
}

function findMessage(messageId) {
  if (!currentRoom) return null;
  return joinedRooms.get(currentRoom)?.msgs.find(msg => msg.message_id === messageId) || null;
}

function canModifyMessage(msg) {
  return msg.message_id && ["message", "media", "edited"].includes(msg.msg_type);
}

function upsertRoomMessage(room, msg) {
  if (!joinedRooms.has(room)) {
    const savedLock = loadRoomLockState(room, typeof username !== "undefined" ? username : "");
    joinedRooms.set(room, { msgs: [], locked: savedLock.locked, passwordHash: savedLock.passwordHash });
  }
  const data = joinedRooms.get(room);
  const id   = msg.message_id;
  if (!id) { data.msgs.push(msg); return false; }
  const existingIndex = data.msgs.findIndex(item => item.message_id === id);
  if (existingIndex >= 0) { data.msgs[existingIndex] = msg; return true; }
  data.msgs.push(msg);
  return false;
}

function previewMessage(msg) {
  if (msg.msg_type === "deleted") return "[da xoa]";
  if (msg.media) return `[${msg.media.media_type}] ${msg.content || msg.media.file_name}`;
  return msg.content || "";
}