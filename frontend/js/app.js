// ─────────────────────────────────────────
// app.js — Global state, WebSocket, Auth
// ─────────────────────────────────────────

let ws = null;
let username = "";
let currentRoom = null;
let joinedRooms = new Map();
let lastUser = "";
let editingMessageId = "";
let suppressCloseAlert = false;
let isLoginMode = true;
let activePanel = "";
let currentUserRole = "";
let currentUserAvatar = "";
let currentUserDisplayName = "";
let userGroups = new Map();

// DOM references
const chatWindow      = document.getElementById("chat-window");
const roomTitle       = document.getElementById("room-title");
const roomList        = document.getElementById("room-list");
const messageInput    = document.getElementById("message-input");
const mediaPanel      = document.getElementById("media-panel");
const mediaFile       = document.getElementById("media-file");
const selectedFile    = document.getElementById("selected-file");
const editBar         = document.getElementById("edit-bar");
const editLabel       = document.getElementById("edit-label");
const loginScreen     = document.getElementById("login");
const mainApp         = document.getElementById("main-app");
const sidebar         = document.getElementById("sidebar");
const chatScreen      = document.getElementById("chat-screen");
const utilityPanel    = document.getElementById("utility-panel");
const panelTitle      = document.getElementById("panel-title");
const panelBody       = document.getElementById("panel-body");
const btnLock         = document.getElementById("btn-lock");
const btnChangeLock   = document.getElementById("btn-change-lock");
const lockModal       = document.getElementById("lock-modal");
const lockModalTitle  = document.getElementById("lock-modal-title");
const lockCurrentRow  = document.getElementById("lock-current-row");
const lockNewRow      = document.getElementById("lock-new-row");
const lockConfirmRow  = document.getElementById("lock-confirm-row");
const lockModalError  = document.getElementById("lock-modal-error");
const lockCurrentPassword = document.getElementById("lock-current-password");
const lockNewPassword     = document.getElementById("lock-new-password");
const lockConfirmPassword = document.getElementById("lock-confirm-password");
const adminPanelBtn   = document.getElementById("admin-panel-btn");

let lockModalMode = "";

// ── Init ──────────────────────────────────
applySavedSettings();

// ── Auth ──────────────────────────────────
function toggleAuthMode() {
  isLoginMode = !isLoginMode;
  const panelLogin    = document.getElementById("panel-login");
  const panelRegister = document.getElementById("panel-register");
  if (!panelLogin || !panelRegister) return;

  if (isLoginMode) {
    panelRegister.classList.remove("active");
    setTimeout(() => {
      panelLogin.classList.add("active");
      document.getElementById("username-input")?.focus();
    }, 50);
    backRegStep1();
  } else {
    panelLogin.classList.remove("active");
    setTimeout(() => {
      panelRegister.classList.add("active");
      document.getElementById("reg-email")?.focus();
    }, 50);
  }
  // Clear all errors
  ["auth-error","reg-error-1","reg-error-2"].forEach(id => {
    const el = document.getElementById(id);
    if (el) el.style.display = "none";
  });
}


function goRegStep2() {
  const email    = document.getElementById("reg-email")?.value.trim();
  const uname    = document.getElementById("reg-username")?.value.trim();
  const phone    = document.getElementById("reg-phone")?.value.trim();
  const errEl    = document.getElementById("reg-error-1");
  const errTxtEl = document.getElementById("reg-error-1-text");

  const showErr = msg => {
    if (errTxtEl) errTxtEl.textContent = msg;
    if (errEl)    errEl.style.display = "flex";
  };

  if (!email || !uname || !phone) { showErr("Vui lòng điền đầy đủ thông tin"); return; }
  if (!email.includes("@") || !email.includes(".")) { showErr("Email không hợp lệ"); return; }
  if (!/^\d{10,20}$/.test(phone.replace(/[+\s-]/g, ""))) { showErr("Số điện thoại không hợp lệ (10–20 chữ số)"); return; }
  if (uname.length < 3) { showErr("Tên đăng nhập phải có ít nhất 3 ký tự"); return; }

  if (errEl) errEl.style.display = "none";

  // Go to step 2
  document.getElementById("reg-step-1").style.display = "none";
  document.getElementById("reg-step-2").style.display = "block";
  document.getElementById("step-dot-1")?.classList.remove("active");
  document.getElementById("step-dot-1")?.classList.add("done");
  document.getElementById("step-dot-2")?.classList.add("active");
  document.querySelector(".reg-step-line")?.classList.add("done");
  setTimeout(() => document.getElementById("reg-password")?.focus(), 80);
}

function backRegStep1() {
  const s1 = document.getElementById("reg-step-1");
  const s2 = document.getElementById("reg-step-2");
  if (s1) s1.style.display = "block";
  if (s2) s2.style.display = "none";
  document.getElementById("step-dot-1")?.classList.add("active");
  document.getElementById("step-dot-1")?.classList.remove("done");
  document.getElementById("step-dot-2")?.classList.remove("active");
  document.querySelector(".reg-step-line")?.classList.remove("done");
  const errEl = document.getElementById("reg-error-2");
  if (errEl) errEl.style.display = "none";
}

function _showFieldError(inputId, message) {
  const input = document.getElementById(inputId);
  if (!input) return;
  input.classList.add("field-error");
  input.focus();
  setTimeout(() => input.classList.remove("field-error"), 820);
  
  const authError = document.getElementById("auth-error");
  const errTxt = document.getElementById("auth-error-text");
  if (authError && errTxt) {
    errTxt.textContent = message;
    authError.style.display = "flex";
  }
}

// Password show/hide toggle
function togglePwd(inputId, btn) {
  const inp = document.getElementById(inputId);
  if (!inp) return;
  const isPass = inp.type === "password";
  inp.type = isPass ? "text" : "password";
  const eyeOff = btn.querySelector(".eye-off");
  const eyeOn  = btn.querySelector(".eye-on");
  if (eyeOff) eyeOff.style.display = isPass ? "none" : "";
  if (eyeOn)  eyeOn.style.display  = isPass ? "" : "none";
}

// Password strength
function checkPasswordStrength(pwd) {
  const bar   = document.getElementById("pwd-strength-bar");
  const label = document.getElementById("pwd-strength-label");
  if (!bar || !label) return;

  let score = 0;
  if (pwd.length >= 6) score++;
  if (pwd.length >= 10) score++;
  if (/[A-Z]/.test(pwd)) score++;
  if (/[0-9]/.test(pwd)) score++;
  if (/[^A-Za-z0-9]/.test(pwd)) score++;

  const levels = [
    { pct: "0%",   color: "transparent", text: "" },
    { pct: "25%",  color: "#ef4444",     text: "Rất yếu" },
    { pct: "50%",  color: "#f97316",     text: "Yếu" },
    { pct: "75%",  color: "#eab308",     text: "Trung bình" },
    { pct: "90%",  color: "#22c55e",     text: "Mạnh" },
    { pct: "100%", color: "#16a34a",     text: "Rất mạnh 💪" },
  ];
  const l = levels[score] || levels[0];
  bar.style.width    = l.pct;
  bar.style.background = l.color;
  label.textContent  = l.text;
  label.style.color  = l.color;
}

async function handleAuth(mode) {
  const authError = document.getElementById("auth-error");
  authError.style.display = "none";
  
  // Remove all previous error classes
  document.querySelectorAll(".field-error").forEach(el => el.classList.remove("field-error"));

  let user, pass, email, phone;

  if (mode === "login") {
    user = document.getElementById("username-input").value.trim();
    pass = document.getElementById("password-input").value.trim();
    
    if (!user) {
      _showFieldError("username-input", "Tên đăng nhập không được để trống");
      return;
    }
    if (!pass) {
      _showFieldError("password-input", "Mật khẩu không được để trống");
      return;
    }
    username = user;
  } else {
    user  = document.getElementById("reg-username").value.trim();
    email = document.getElementById("reg-email").value.trim();
    phone = document.getElementById("reg-phone").value.trim();
    pass  = document.getElementById("reg-password").value.trim();
    const passConfirm = document.getElementById("reg-password-confirm").value.trim();

    if (!pass) {
      _showFieldError("reg-password", "Vui lòng nhập mật khẩu");
      return;
    }
    if (!passConfirm) {
      _showFieldError("reg-password-confirm", "Vui lòng xác nhận mật khẩu");
      return;
    }
    if (pass !== passConfirm) {
      _showFieldError("reg-password-confirm", "Mật khẩu không trùng khớp");
      return;
    }
    if (!email || !email.includes("@") || !email.includes(".")) {
      _showFieldError("reg-email", "Email không hợp lệ");
      return;
    }
    if (!phone || !/^\d{10,20}$/.test(phone.replace(/\+/g, ""))) {
      _showFieldError("reg-phone", "Số điện thoại không hợp lệ (10-20 chữ số)");
      return;
    }
    if (!user) {
      _showFieldError("reg-username", "Tên đăng nhập không được để trống");
      return;
    }
  }

  if (!ws || ws.readyState !== WebSocket.OPEN) {
    try {
      await connectWebSocket();
    } catch (error) {
      console.error(error);
      const errTxt6 = document.getElementById("auth-error-text");
      if (errTxt6) errTxt6.textContent = "Không thể kết nối tới server. Vui lòng kiểm tra server đang chạy.";
      authError.style.display = "flex";
      return;
    }
    ws.onmessage = handleServerMessage;
    ws.onclose = () => {
      if (!suppressCloseAlert) showToast("Mất kết nối tới server", "error");
      resetApp();
    };
  }

  sendAuthRequest(mode, user, pass, email, phone);
}

function sendAuthRequest(mode, user, pass, email, phone) {
  ws.send(JSON.stringify({
    msg_type: mode,
    username: user,
    password: pass,
    email:    email || "",
    phone:    phone || "",
    content:  mode === "register" ? "Đăng ký mới" : "Đăng nhập",
    room:     "general",
    target:   "",
    users:    []
  }));
}

// ── WebSocket ─────────────────────────────
function createWebSocket(url) {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(url);
    socket.onopen  = () => resolve(socket);
    socket.onerror = () => reject(new Error(`WebSocket lỗi: ${url}`));
    socket.onclose = () => reject(new Error(`WebSocket đóng sớm: ${url}`));
  });
}

async function connectWebSocket() {
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  const hostname  = location.hostname || "127.0.0.1";
  const ports     = [];
  for (let port = 3000; port <= 3010; port++) ports.push(port.toString());
  const currentPort = location.port || "3000";
  if (!ports.includes(currentPort)) ports.push(currentPort);

  let lastError = null;
  for (const port of ports) {
    const url = `${protocol}//${hostname}:${port}/ws`;
    try { ws = await createWebSocket(url); return; }
    catch (error) { lastError = error; }
  }
  throw lastError || new Error("Không thể kết nối WebSocket");
}

function sendAction(msg_type, { content = "", target = "", room = "general" } = {}) {
  if (!ws || ws.readyState !== WebSocket.OPEN) {
    showToast("Mất kết nối tới server", "error");
    return;
  }
  ws.send(JSON.stringify({ msg_type, username, password: "", content, room, target, users: [] }));
}

// ── Message handler ───────────────────────
function handleServerMessage(e) {
  const data = JSON.parse(e.data);

  if (handleControlMessage(data)) return;

  // ── Realtime avatar broadcast từ server ──
  // Server emit khi bất kỳ user nào đổi avatar
  if (data.msg_type === "avatar_updated" || data.msg_type === "user_avatar_updated" || data.msg_type === "profile_broadcast") {
    const who = data.username || data.content?.username;
    const url  = data.avatar_url || data.content?.avatar_url || (typeof data.content === "string" ? tryParseJson(data.content)?.avatar_url : null);
    if (who && url) {
      window._avatarCache = window._avatarCache || {};
      window._avatarCache[who] = url;
      if (typeof syncAvatarsForUser === "function") syncAvatarsForUser(who, url);
      if (typeof renderSidebar === "function") renderSidebar();
    }
    return;
  }

  if (data.msg_type === "error") {
    showToast(data.content || "Lỗi từ server", "error");
    return;
  }

  if (data.msg_type === "system" && data.content.toLowerCase().includes("đăng nhập thành công")) {
    loginScreen.classList.add("hidden");
    if (mainApp) mainApp.classList.remove("hidden");
    requestDashboardData();
    sendAction("profile_get");
    return;
  }

  if (data.msg_type === "system" && data.content.toLowerCase().includes("đăng ký thành công")) {
    showToast(data.content, "success");
    if (!isLoginMode) toggleAuthMode();
    return;
  }

  if (!joinedRooms.has(data.room)) {
    const savedLock = loadRoomLockState(data.room, typeof username !== "undefined" ? username : "");
    joinedRooms.set(data.room, { msgs: [], locked: savedLock.locked, passwordHash: savedLock.passwordHash });
    // Lưu room vào localStorage để persist sau login lại — kể cả private chat rooms
    if (typeof saveRoomToLocalStorage === "function") {
      saveRoomToLocalStorage(data.room);
    }
  }

  // Nếu message có kèm avatar_url của sender — cache lại ngay
  if (data.username && data.avatar_url) {
    window._avatarCache = window._avatarCache || {};
    const cached = window._avatarCache[data.username];
    if (!cached || cached !== data.avatar_url) {
      window._avatarCache[data.username] = data.avatar_url;
      if (typeof syncAvatarsForUser === "function") {
        syncAvatarsForUser(data.username, data.avatar_url);
      }
    }
  }

  const isUpdate = upsertRoomMessage(data.room, data);
  if (data.room !== currentRoom && !isUpdate && (data.msg_type === "message" || data.msg_type === "media")) {
    const rData = joinedRooms.get(data.room);
    if (rData) {
      rData.unread = true;
      playNotificationSound();
    }
  }
  renderSidebar();

  if (data.room === currentRoom) {
    if (isRoomLocked(currentRoom)) {
      renderMessages();
    } else {
      isUpdate ? renderMessages() : renderMessage(data);
    }
  }
}

// ── App reset ─────────────────────────────
function resetApp() {
  ws = null;
  username = "";
  currentRoom = null;
  joinedRooms = new Map();
  lastUser = "";
  currentUserRole = "";
  currentUserAvatar = "";
  currentUserDisplayName = "";
  userGroups.clear();
  suppressCloseAlert = false;
  adminPanelBtn.classList.add("hidden");
  roomList.innerHTML = "";
  chatWindow.innerHTML = '<div class="empty"></div>';
  messageInput.disabled = false;
  closePanel();
  loginScreen.classList.remove("hidden");
  if (mainApp) mainApp.classList.add("hidden");
  // Reset auth to login panel
  const panelLogin    = document.getElementById("panel-login");
  const panelRegister = document.getElementById("panel-register");
  if (panelRegister) panelRegister.classList.remove("active");
  if (panelLogin)    panelLogin.classList.add("active");
  backRegStep1();
  isLoginMode = true;
  document.getElementById("auth-error").style.display = "none";

  isLoginMode = true;
  const loginForm = document.getElementById("login-form");
  const regForm   = document.getElementById("register-form");
  if (loginForm && regForm) {
    loginForm.style.display = "block";
    regForm.style.display   = "none";
  }
  const title      = document.getElementById("auth-title");
  if (title)       title.innerText = "Rust Chat App";
  const desc       = document.getElementById("auth-desc");
  if (desc)        desc.innerText  = "Vui lòng đăng nhập hệ thống";
  const btnPrimary = document.getElementById("btn-primary");
  if (btnPrimary)  btnPrimary.innerText = "Đăng nhập";
  const toggleLink = document.getElementById("toggle-link");
  if (toggleLink)  toggleLink.innerText = "Chưa có tài khoản? Đăng ký ngay";
}

function logout() {
  suppressCloseAlert = true;
  if (ws && ws.readyState === WebSocket.OPEN) sendAction("logout");
  if (ws) ws.close();
  resetApp();
}

// ── Settings ──────────────────────────────
function applySavedSettings() {
  const theme = localStorage.getItem("chat-theme") || "light";
  document.body.classList.toggle("theme-dark", theme === "dark");
  document.body.classList.toggle("theme-light", theme !== "dark");
  // Sync nav status dot border color
  document.querySelectorAll(".nav-status-dot").forEach(d => {
    d.style.borderColor = ""; // let CSS handle it via var(--nav-bg)
  });
}

function saveUiSettings() {
  const sel = document.getElementById("setting-theme");
  if (sel) localStorage.setItem("chat-theme", sel.value);
  const snd = document.getElementById("setting-sound");
  if (snd) localStorage.setItem("chat-sound", snd.value);
  applySavedSettings();
  // Update nav avatar in case profile changed
  updateNavAvatar();
}

function updateNavAvatar() {
  const el = document.getElementById("nav-avatar-el");
  if (!el) return;
  if (currentUserAvatar) {
    el.innerHTML = `<img src="${currentUserAvatar}" style="width:100%;height:100%;object-fit:cover;border-radius:50%;">`;
  } else {
    el.textContent = (username || "?")[0].toUpperCase();
  }
}

function changePassword() {
  const oldPassword = document.getElementById("old-password").value;
  const newPassword = document.getElementById("new-password").value;
  if (!oldPassword || !newPassword) return showToast("Vui lòng nhập đầy đủ mật khẩu", "warning");
  sendAction("settings_change_password", {
    content: JSON.stringify({ old_password: oldPassword, new_password: newPassword })
  });
}

// ── Utilities ─────────────────────────────
function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/g, c =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#039;" }[c])
  );
}
function escapeAttr(value) { return escapeHtml(value).replace(/`/g, "&#096;"); }
function escapeJs(value)   { return String(value ?? "").replace(/\\/g, "\\\\").replace(/'/g, "\\'"); }

const _URL_RE = /https?:\/\/[^\s<>"']+[^\s<>"'.,:;!?)\]]/g;
function linkifyContent(text) {
  const frag = document.createDocumentFragment();
  let last = 0;
  for (const m of text.matchAll(_URL_RE)) {
    if (m.index > last) frag.appendChild(document.createTextNode(text.slice(last, m.index)));
    const a = document.createElement("a");
    a.href = m[0]; a.textContent = m[0]; a.target = "_blank"; a.rel = "noopener noreferrer";
    frag.appendChild(a);
    last = m.index + m[0].length;
  }
  if (last < text.length) frag.appendChild(document.createTextNode(text.slice(last)));
  return frag;
}

function formatBytes(bytes) {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / Math.pow(1024, index)).toFixed(index ? 1 : 0)} ${units[index]}`;
}

function formatMessageTime(timestamp) {
  const date = new Date(timestamp * 1000);
  if (Number.isNaN(date.getTime())) return "";
  const now  = new Date();
  const hhmm = `${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
  const todayStart     = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterdayStart = new Date(todayStart - 86400000);
  const weekStart      = new Date(todayStart - 6 * 86400000);
  const dateStart      = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  if (dateStart.getTime() === todayStart.getTime())     return hhmm;
  if (dateStart.getTime() === yesterdayStart.getTime()) return `Hôm qua ${hhmm}`;
  if (dateStart >= weekStart) {
    const days = ["CN", "T2", "T3", "T4", "T5", "T6", "T7"];
    return `${days[date.getDay()]} ${hhmm}`;
  }
  return `${pad2(date.getDate())}/${pad2(date.getMonth() + 1)}/${date.getFullYear()} ${hhmm}`;
}

function formatAdminDate(value) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return `${pad2(date.getDate())}/${pad2(date.getMonth() + 1)}/${date.getFullYear()} ${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
}

function pad2(value) { return String(value).padStart(2, "0"); }

function detectMediaType(file) {
  if (file.type.startsWith("image/")) return "image";
  if (file.type.startsWith("video/")) return "video";
  return "file";
}

function requestDashboardData() {
  sendAction("profile_get");
  sendAction("friends_list");
  sendAction("groups_list");
  // Restore previously joined rooms (cả group lẫn private chat) cho user hiện tại
  const savedRooms = typeof getSavedRooms === "function" ? getSavedRooms() : [];
  savedRooms.forEach(id => {
    if (!joinedRooms.has(id)) joinRoom(id);
  });
  // Restore lock states — delay để đảm bảo profile_get đã xử lý xong
  // Gọi sau khi username đã được set từ profile_get response (~400ms)
  const tryRestoreLockStates = () => {
    if (username && typeof restoreAllLockStates === "function") {
      restoreAllLockStates();
    } else {
      // Thử lại nếu username chưa có (login bằng email/phone)
      setTimeout(tryRestoreLockStates, 600);
    }
  };
  setTimeout(tryRestoreLockStates, 400);
}

function restoreAllLockStates() {
  // Chỉ restore lock state của USER HIỆN TẠI
  // Key format mới: "chat_lock_u:USERNAME:room"
  if (!username) return; // chưa có username → không restore

  const prefix = `chat_lock_u:${username}:`;
  let restored = 0;

  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (!key || !key.startsWith(prefix)) continue;
    try {
      const room  = key.slice(prefix.length);
      const saved = JSON.parse(localStorage.getItem(key));
      if (!saved || !saved.passwordHash) continue;
      if (joinedRooms.has(room)) {
        const data    = joinedRooms.get(room);
        data.locked       = !!saved.locked;
        data.passwordHash = saved.passwordHash;
        joinedRooms.set(room, data);
        restored++;
      }
    } catch(e) {}
  }

  // Migration: đọc key cũ (chat_lock_room) nếu có và chưa có key mới
  const oldPrefix = "chat_lock_";
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i);
    if (!key || !key.startsWith(oldPrefix) || key.startsWith("chat_lock_u:")) continue;
    try {
      const room    = key.slice(oldPrefix.length);
      const newKey  = `chat_lock_u:${username}:${room}`;
      if (localStorage.getItem(newKey)) continue; // đã có key mới, bỏ qua
      const saved   = JSON.parse(localStorage.getItem(key));
      if (!saved || !saved.passwordHash) continue;
      // Migrate sang key mới
      localStorage.setItem(newKey, JSON.stringify(saved));
      localStorage.removeItem(key);
      if (joinedRooms.has(room)) {
        const data    = joinedRooms.get(room);
        data.locked       = !!saved.locked;
        data.passwordHash = saved.passwordHash;
        joinedRooms.set(room, data);
        restored++;
      }
    } catch(e) {}
  }

  if (restored > 0) {
    if (typeof renderSidebar === "function") renderSidebar();
    if (typeof updateLockActions === "function") updateLockActions();
  }
}
function tryParseJson(str) {
  try { return JSON.parse(str); } catch { return null; }
}

function playNotificationSound() {
  if (localStorage.getItem("chat-sound") !== "on") return;
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.type = "sine";
    osc.frequency.setValueAtTime(587.33, ctx.currentTime); // D5
    osc.frequency.setValueAtTime(880.00, ctx.currentTime + 0.08); // A5
    gain.gain.setValueAtTime(0.08, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.35);
    osc.start(ctx.currentTime);
    osc.stop(ctx.currentTime + 0.35);
  } catch (e) {
    console.warn("Audio context failed:", e);
  }
}