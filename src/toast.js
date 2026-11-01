// Dock'un üstünde beliren bildirim balonları. Her bildirim 6 sn kalır (fare
// üstündeyken bekler); tıklayınca uygulama öne gelir.
"use strict";

const TAURI = window.__TAURI__;
const invoke = TAURI ? TAURI.core.invoke : mockInvoke;
const listen = TAURI ? TAURI.event.listen : async () => () => {};
const stack = document.getElementById("stack");
const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const LIFE = 6000;
const MAX = 3;
let queue = [];   // görünenler: { p, el, until }
let hidden = 0;   // sığmayan bildirim sayısı
let tcfg = null;

async function applyTheme() {
  tcfg = await invoke("get_config").catch(() => null);
  if (!tcfg) return;
  const root = document.documentElement;
  const dark = !!(await invoke("theme_dark").catch(() => true));
  root.dataset.theme = dark ? "dark" : "light";
  root.dataset.backdrop = tcfg.backdrop;
  root.style.setProperty("--accent", tcfg.accent || "#2F7BFF");
  await Dyn.apply(root, tcfg, dark, "popover");
}

function relayout() {
  requestAnimationFrame(() => {
    // Animasyon küçültmesinden etkilenmeyen gerçek boyut
    invoke("toast_layout", { width: stack.offsetWidth + 1, height: stack.offsetHeight + 1 }).catch(() => {});
  });
}

function timeTxt(ms) {
  if (!ms) return "şimdi";
  const m = Math.round((Date.now() - ms) / 60000);
  return m < 1 ? "şimdi" : `${m} dk önce`;
}

function card(p) {
  const b = document.createElement("button");
  b.className = "tn";
  b.style.setProperty("--life", `${p.life || LIFE}ms`);
  b.innerHTML = `
    <span class="ic">${p.icon ? `<img src="${p.icon}" alt="">` : esc((p.name || "?").slice(0, 1))}</span>
    <span class="tx">
      <span class="hd"><b>${esc(p.name)}</b><span>${timeTxt(p.time)}</span></span>
      <span class="tt">${esc(p.title || p.name)}</span>
      ${p.body ? `<span class="bd">${esc(p.body)}</span>` : ""}
    </span>
    <span class="x" role="button" aria-label="Kapat"><svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg></span>
    <span class="life"></span>`;
  b.title = "Açmak için tıkla";
  b.onclick = (e) => {
    if (e.target.closest(".x")) return remove(p.id);
    if (p.key) invoke("toast_click", { key: p.key }).catch(() => {});
    else invoke("notif_open_center").catch(() => {});
    clearAll();
  };
  return b;
}

function draw() {
  stack.querySelector(".more")?.remove();
  if (hidden > 0) {
    const m = document.createElement("div");
    m.className = "more";
    m.textContent = `+${hidden} bildirim daha`;
    stack.append(m);
  }
  relayout();
}

async function add(p) {
  if (!tcfg) await applyTheme();
  if (queue.some((q) => q.p.id === p.id)) return;
  const el = card(p);
  stack.prepend(el);
  queue.unshift({ p, el, until: Date.now() + (p.life || LIFE) });
  while (queue.length > MAX) {
    const old = queue.pop();
    old.el.remove();
    hidden++;
  }
  stack.classList.add("in");
  draw();
}

function remove(id) {
  const i = queue.findIndex((q) => q.p.id === id);
  if (i < 0) return;
  const [q] = queue.splice(i, 1);
  q.el.classList.add("out");
  setTimeout(() => {
    q.el.remove();
    if (!queue.length) finish();
    else draw();
  }, 240);
}

function clearAll() {
  queue = [];
  finish();
}

function finish() {
  hidden = 0;
  stack.classList.remove("in");
  setTimeout(() => {
    if (queue.length) return;
    stack.replaceChildren();
    invoke("toast_hide").catch(() => {});
  }, 260);
}

// Süresi dolanları kaldır (fare üstündeyken bekle)
let hovering = false;
stack.addEventListener("mouseenter", () => (hovering = true));
stack.addEventListener("mouseleave", () => {
  hovering = false;
  for (const q of queue) q.until = Math.max(q.until, Date.now() + 2000);
});
setInterval(() => {
  if (hovering) { for (const q of queue) q.until += 250; return; }
  for (const q of [...queue]) if (Date.now() >= q.until) remove(q.p.id);
}, 250);

listen("toast-add", (e) => add(e.payload));
listen("config-changed", applyTheme);
listen("theme-dark", applyTheme);
(async () => {
  await applyTheme();
  const p = await invoke("toast_pending").catch(() => null);
  if (p) add(p);
})();

// Tarayıcı önizlemesi: toast.html?theme=dark
async function mockInvoke(cmd, args) {
  const q = new URLSearchParams(location.search);
  if (cmd === "get_config") return { theme: q.get("theme") || "dark", backdrop: "acrylic", accent: "#2F7BFF", dynamic: { enabled: q.has("wall"), tint: 45 } };
  if (cmd === "theme_dark") return (q.get("theme") || "dark") === "dark";
  if (cmd === "wallpaper") return MOCK.wallpaper(q.get("wall"));
  if (cmd === "toast_pending") {
    setTimeout(() => add({ id: 2, key: "", name: "Microsoft Teams", icon: null, title: "Proje kanalı", body: "Dashboard'un son hali hazır, bakabilir misin?", time: Date.now() - 60000 }), 150);
    setTimeout(() => add({ id: 3, key: "", name: "Outlook", icon: null, title: "Haftalık BI raporu", body: "Ekte bu haftanın KPI özetini bulabilirsin.", time: Date.now() }), 300);
    return { id: 1, key: "", name: "WhatsApp", icon: null, title: "Aile grubu", body: "Akşam yemeğe kaçta geliyorsun? 🙂", time: Date.now() - 120000 };
  }
  return null;
}
