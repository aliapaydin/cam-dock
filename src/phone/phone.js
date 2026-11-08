// Cam Dock telefon ekranı. Cam Dock'un içindeki sunucudan gelir; veriler
// WebSocket ile akar. Bağlantı koparsa kendiliğinden yeniden dener.
"use strict";

const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const el = (tag, cls, html) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (html != null) e.innerHTML = html;
  return e;
};
const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const Native = window.Android || null;
const css = (v) => getComputedStyle(document.body).getPropertyValue(v).trim();

const ICON = {
  prev: '<svg viewBox="0 0 24 24"><path d="M6 5h2v14H6zM20 5v14L9.5 12z"/></svg>',
  next: '<svg viewBox="0 0 24 24"><path d="M16 5h2v14h-2zM4 5v14l10.5-7z"/></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="M7 4.5v15l13-7.5z"/></svg>',
  pause: '<svg viewBox="0 0 24 24"><path d="M6 4h4.5v16H6zM13.5 4H18v16h-4.5z"/></svg>',
  vol: '<svg viewBox="0 0 24 24"><path d="M3 9v6h4l5 4V5L7 9zm13.5 3a4.5 4.5 0 0 0-2.5-4v8a4.5 4.5 0 0 0 2.5-4z"/></svg>',
  mute: '<svg viewBox="0 0 24 24"><path d="M3 9v6h4l5 4V5L7 9zm16.6 3 2.1-2.1-1.4-1.4-2.1 2.1-2.1-2.1-1.4 1.4 2.1 2.1-2.1 2.1 1.4 1.4 2.1-2.1 2.1 2.1 1.4-1.4z"/></svg>',
};
const KEY_COLORS = ["#64748b", "#3b82f6", "#0ea5e9", "#eab308", "#22c55e", "#a855f7", "#14b8a6", "#22c55e", "#f59e0b", "#8b5cf6", "#ef4444", "#94a3b8"];

// ---------------------------------------------------------------------------
// Durum
// ---------------------------------------------------------------------------
const HMAX = 90;
const S = {
  cfg: null,
  stats: null,
  sys: null,
  claude: null,
  icons: {},
  hist: { cpu: [], gpu: [], ram: [], down: [], up: [] },
  media: { title: "", artist: "", playing: false, pos: 0, dur: 0, cover: null, at: Date.now() },
  vol: { v: 50, m: false },
  wx: null,
  aq: null,
  phone: null,
};

(() => {
  const u = new URL(location.href);
  const t = u.searchParams.get("token");
  if (t) {
    try { localStorage.setItem("camdock-token", t); } catch {}
    history.replaceState(null, "", "/");
  }
})();
const store = {
  get: (k, d = "") => { try { return localStorage.getItem(k) ?? d; } catch { return d; } },
  set: (k, v) => { try { localStorage.setItem(k, v); } catch {} },
};
const deviceName = () => { try { return Native?.deviceName?.() || "Telefon"; } catch { return "Telefon"; } };

// ---------------------------------------------------------------------------
// Bağlantı
// ---------------------------------------------------------------------------
let ws = null;
let fails = 0;
let retryTimer = null;

function connect() {
  clearTimeout(retryTimer);
  try { ws?.close(); } catch {}
  ws = new WebSocket(`ws://${location.host}/ws`);
  ws.onopen = () => {
    fails = 0;
    send({ t: "hello", token: store.get("camdock-token"), device: deviceName() });
  };
  ws.onmessage = (e) => {
    let m;
    try { m = JSON.parse(e.data); } catch { return; }
    onMsg(m);
  };
  ws.onclose = () => {
    showConn(true);
    fails++;
    if (fails >= 3) { try { Native?.connectionLost?.(); } catch {} }
    retryTimer = setTimeout(connect, Math.min(4000, 600 + fails * 500));
  };
}
const send = (o) => { if (ws && ws.readyState === 1) ws.send(JSON.stringify(o)); };

function showConn(on, text = "Bilgisayara bağlanıyor…") {
  const c = $("#conn");
  c.querySelector("span").textContent = text;
  c.classList.toggle("show", on);
}

let toastTimer = null;
function toast(text, ms = 2200) {
  const t = $("#toast");
  t.textContent = text;
  t.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove("show"), ms);
}

function onMsg(m) {
  switch (m.t) {
    case "auth":
      if (m.ok) {
        if (m.token) store.set("camdock-token", m.token);
        $("#pair").hidden = true;
        showConn(false);
        try { Native?.connected?.(); } catch {}
      } else {
        showConn(false);
        showPair(m.pc, m.wrong);
      }
      break;
    case "hist":
      for (const h of m.items || []) { push(S.hist.cpu, h.cpu); push(S.hist.gpu, h.gpu); push(S.hist.ram, h.ram); }
      break;
    case "cfg": applyCfg(m); break;
    case "stats":
      S.stats = m;
      push(S.hist.cpu, m.cpu); push(S.hist.gpu, m.gpu); push(S.hist.ram, m.ram); push(S.hist.down, m.down); push(S.hist.up, m.up);
      renderStats();
      break;
    case "sys":
      S.sys = m;
      Object.assign(S.icons, m.icons || {});
      renderSys();
      break;
    case "claude":
      S.claude = m;
      renderRings();
      break;
    case "media": {
      const cover = "cover" in m ? m.cover : S.media.cover;
      S.media = { ...m, cover, at: Date.now() };
      renderMedia();
      break;
    }
    case "vol":
      S.vol = m;
      renderVol();
      break;
    case "page":
      goToKey(m.page);
      break;
  }
}
const push = (a, v) => { a.push(Number(v) || 0); if (a.length > HMAX) a.shift(); };

// ---------------------------------------------------------------------------
// Eşleşme
// ---------------------------------------------------------------------------
function showPair(pc, wrong) {
  const p = $("#pair");
  p.hidden = false;
  $("#pair-pc").textContent = pc || "Cam Dock";
  $("#pair-err").textContent = wrong ? "Kod yanlış, yeniden dene." : "";
  if (wrong) {
    const c = p.querySelector(".pair-card");
    c.classList.remove("shake"); void c.offsetWidth; c.classList.add("shake");
    $("#pin").value = "";
  }
  $("#pair-other").hidden = !Native?.chooseOther;
  setTimeout(() => $("#pin").focus(), 300);
}
$("#pin").addEventListener("input", (e) => {
  e.target.value = e.target.value.replace(/\D/g, "").slice(0, 6);
  if (e.target.value.length === 6) pairGo();
});
$("#pair-go").onclick = () => pairGo();
$("#pair-other").onclick = () => { try { Native?.chooseOther?.(); } catch {} };
function pairGo() {
  const pin = $("#pin").value;
  if (pin.length !== 6) { $("#pair-err").textContent = "6 haneli kodu gir."; return; }
  send({ t: "pair", pin, device: deviceName() });
}

// ---------------------------------------------------------------------------
// Tema
// ---------------------------------------------------------------------------
const THEMES = { pc: "Bilgisayarla aynı", dark: "Koyu", light: "Açık", amoled: "AMOLED", midnight: "Gece mavisi" };
function applyTheme() {
  const pick = store.get("camdock-theme", "") || S.cfg?.theme || "pc";
  let t = pick === "pc" ? (S.cfg?.dark === false ? "light" : "dark") : pick;
  document.body.className = document.body.className.replace(/\bt-\w+/g, "").trim();
  document.body.classList.add(`t-${t}`);
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", css("--bg") || "#06080b");
  renderStats();
}

// ---------------------------------------------------------------------------
// Sayfalar
// ---------------------------------------------------------------------------
const PAGES = {
  home: { name: "Ana ekran", icon: "🏠", build: homePage },
  dashboard: { name: "Gösterge", icon: "🎛️", build: dashPage },
  system: { name: "Sistem", icon: "📈", build: sysPage },
  status: { name: "Durum", icon: "🧭", build: statusPage },
  ambient: { name: "Ortam", icon: "🌙", build: ambientPage },
  media: { name: "Müzik", icon: "🎵", build: mediaPage },
  shortcuts: { name: "Kısayollar", icon: "⌨️", build: keysPage },
  clock: { name: "Saat", icon: "🕰️", build: clockPage },
};
const ALIAS = { stats: "system" };
let pagesKey = "";
let pageList = [];

function applyCfg(c) {
  const first = !S.cfg;
  S.cfg = c;
  document.documentElement.style.setProperty("--accent", c.accent || "#2f7bff");
  applyTheme();
  const key = JSON.stringify([c.pages, c.shortcuts, c.cpu_name, c.gpu_name]);
  if (key !== pagesKey) {
    pagesKey = key;
    buildPages();
  }
  if (first) loadWeather();
}

function buildPages() {
  const root = $("#pages");
  const cur = currentIndex();
  root.replaceChildren();
  pageList = (S.cfg.pages || []).map((p) => ALIAS[p] || p).filter((p) => PAGES[p]);
  if (!pageList.length) pageList = ["home"];
  for (const p of pageList) {
    const pg = PAGES[p].build();
    pg.dataset.key = p;
    root.append(pg);
  }
  const dots = $("#dots");
  dots.replaceChildren(...pageList.map(() => el("i")));
  root.scrollLeft = Math.min(cur, pageList.length - 1) * root.clientWidth;
  updateDots();
  renderAll();
}

const currentIndex = () => { const r = $("#pages"); return Math.round(r.scrollLeft / Math.max(1, r.clientWidth)); };
function goTo(i, smooth = true) {
  const r = $("#pages");
  i = Math.max(0, Math.min(pageList.length - 1, i));
  r.scrollTo({ left: i * r.clientWidth, behavior: smooth ? "smooth" : "auto" });
}
function goToKey(k) { const i = pageList.indexOf(ALIAS[k] || k); if (i >= 0) goTo(i); }
window.pageStep = (d) => { goTo(currentIndex() + d); touched(); };

$("#pages").addEventListener("scroll", () => updateDots(), { passive: true });
function updateDots() {
  const i = currentIndex();
  $$("#dots i").forEach((d, k) => d.classList.toggle("on", k === i));
}

// Boşta kalınca ana sayfaya dön; gece ekranı karart
let lastTouch = Date.now();
function touched() { lastTouch = Date.now(); document.body.classList.remove("night"); }
["pointerdown", "keydown", "wheel"].forEach((e) => window.addEventListener(e, touched, { passive: true }));
setInterval(() => {
  const idle = (Date.now() - lastTouch) / 1000;
  const back = S.cfg?.idle_home ?? 120;
  if (back > 0 && idle > back && currentIndex() !== 0 && $("#sheet").hidden) goTo(0);
  const h = new Date().getHours();
  const night = S.cfg?.night_dim !== false && (h >= 23 || h < 7) && idle > 60;
  document.body.classList.toggle("night", night);
}, 5000);

// Noktalara dokununca ya da boş yere uzun basınca: genel bakış
$("#dots").addEventListener("click", openOverview);
let lp = null;
$("#pages").addEventListener("pointerdown", (e) => {
  if (e.target.closest("button, input, .key, .prow, .vslider")) return;
  lp = setTimeout(openOverview, 650);
});
["pointerup", "pointercancel", "pointermove"].forEach((ev) => $("#pages").addEventListener(ev, (e) => {
  if (ev === "pointermove" && Math.abs(e.movementX) + Math.abs(e.movementY) < 4) return;
  clearTimeout(lp);
}));

function openOverview() {
  const sh = $("#sheet");
  sh.replaceChildren();
  const card = el("div", "sheet-card");
  card.append(el("h2", "", "Sayfalar"));
  const ov = el("div", "ov");
  const cur = currentIndex();
  pageList.forEach((k, i) => {
    const b = el("button", `press${i === cur ? " on" : ""}`, `<span class="e">${PAGES[k].icon}</span>${PAGES[k].name}`);
    b.onclick = () => { closeSheet(); goTo(i, false); };
    ov.append(b);
  });
  card.append(ov, el("h2", "", "Tema"));
  const th = el("div", "themes");
  const pick = store.get("camdock-theme", "") || "pc";
  for (const [k, v] of Object.entries(THEMES)) {
    const b = el("button", `press${k === pick ? " on" : ""}`, v);
    b.onclick = () => { store.set("camdock-theme", k); applyTheme(); openOverview(); };
    th.append(b);
  }
  card.append(th);
  sh.append(card);
  sh.hidden = false;
  sh.onclick = (e) => { if (e.target === sh) closeSheet(); };
}
const closeSheet = () => { $("#sheet").hidden = true; };

// --- Yapı taşları ---------------------------------------------------------
function tile(kind, label, color) {
  const t = el("div", `card tile m-${kind}`);
  t.style.setProperty("--c", color);
  t.innerHTML = `<span class="lbl">${label}</span><span class="val">–</span><span class="sub"></span><canvas></canvas>`;
  return t;
}

function ring(kind, label, color) {
  const r = el("div", `card ring r-${kind}`);
  r.style.setProperty("--c", color);
  const C = 2 * Math.PI * 42;
  r.innerHTML = `<svg viewBox="0 0 100 100"><circle class="tr" cx="50" cy="50" r="42" fill="none" stroke-width="8"/>
    <circle class="pv" cx="50" cy="50" r="42" fill="none" stroke-width="8" stroke-dasharray="${C}" stroke-dashoffset="${C}" transform="rotate(-90 50 50)"/>
    <text x="50" y="57" text-anchor="middle" font-size="21"><tspan class="v">–</tspan><tspan class="u" font-size="11"></tspan></text></svg>
    <span class="lbl">${label}</span><span class="sub"></span>`;
  return r;
}
function setRing(r, pct, value, unit, sub) {
  if (!r) return;
  const C = 2 * Math.PI * 42;
  r.querySelector(".pv").style.strokeDashoffset = C * (1 - Math.max(0, Math.min(1, pct / 100)));
  const t = r.querySelector("text");
  t.querySelector(".v").textContent = value;
  t.querySelector(".u").textContent = unit || "";
  t.querySelector(".v").style.fill = "var(--text)";
  if (sub != null) r.querySelector(".sub").textContent = sub;
}

function clockBox(withWx = true) {
  const c = el("div", "clockbox");
  c.innerHTML = `<div class="topline"><i class="dot"></i><b class="pc"></b><span class="bat"></span></div>
    <div class="time"><span class="hh">--</span><span class="colon">:</span><span class="mm">--</span><sup class="ss">00</sup></div>
    <div class="date"></div>${withWx ? '<div class="wx"></div>' : ""}`;
  return c;
}

function mediaCard(big) {
  const b = el("div", big ? "mcard" : "card mcard");
  b.innerHTML = `
    <div class="cover"><span>♪</span></div>
    <div class="mtext"><span class="lbl src">Medya</span><div class="mtitle">Çalan bir şey yok</div><div class="martist"></div></div>
    <div class="prog"><div class="bar"><i></i></div><div class="times"><span class="cur">0:00</span><span class="dur">0:00</span></div></div>
    <div class="ctrls">
      <button class="cbtn press" data-a="prev" aria-label="Önceki">${ICON.prev}</button>
      <button class="cbtn main press" data-a="toggle" aria-label="Oynat/Duraklat">${ICON.play}</button>
      <button class="cbtn press" data-a="next" aria-label="Sonraki">${ICON.next}</button>
      <div class="vol"><button class="cbtn press mutebtn" aria-label="Sessize al">${ICON.vol}</button><input type="range" min="0" max="100" class="volr" aria-label="Ses"><span class="n"></span></div>
    </div>`;
  b.querySelectorAll("[data-a]").forEach((x) => (x.onclick = () => {
    send({ t: "media", a: x.dataset.a });
    if (x.dataset.a === "toggle") { S.media.playing = !S.media.playing; S.media.at = Date.now(); renderMedia(); }
  }));
  wireVolume(b);
  return b;
}

function wireVolume(root) {
  root.querySelectorAll(".volr").forEach((r) => {
    let last = 0;
    r.oninput = () => {
      S.vol.v = Number(r.value);
      renderVol(r);
      const now = Date.now();
      if (now - last > 60) { last = now; send({ t: "vol", v: S.vol.v }); }
    };
    r.onchange = () => send({ t: "vol", v: Number(r.value) });
  });
  root.querySelectorAll(".mutebtn").forEach((x) => (x.onclick = () => { S.vol.m = !S.vol.m; send({ t: "mute", m: S.vol.m }); renderVol(); }));
}

function keyButton(s, i) {
  const b = el("button", "key press");
  b.style.setProperty("--k", s.color || KEY_COLORS[i % KEY_COLORS.length]);
  b.innerHTML = `<span class="e">${esc(s.icon || "⚡")}</span><span class="t">${esc(s.label)}</span>`;
  b.addEventListener("pointerdown", (e) => {
    const r = b.getBoundingClientRect();
    const w = el("i", "rip");
    w.style.left = `${e.clientX - r.left}px`;
    w.style.top = `${e.clientY - r.top}px`;
    b.append(w);
    setTimeout(() => w.remove(), 650);
  });
  b.onclick = () => {
    send({ t: "run", i });
    try { navigator.vibrate?.(12); } catch {}
    b.classList.add("ok");
    setTimeout(() => b.classList.remove("ok"), 500);
  };
  return b;
}

// --- Sayfalar -------------------------------------------------------------
function homePage() {
  const p = el("section", "page p-home");
  const tiles = el("div", "tiles");
  tiles.append(tile("cpu", "İşlemci", "var(--cpu)"), tile("gpu", "Ekran kartı", "var(--gpu)"), tile("ram", "Bellek", "var(--ram)"), tile("net", "Ağ", "var(--net)"));
  p.append(clockBox(), mediaCard(false), tiles);
  return p;
}

function dashPage() {
  const p = el("section", "page p-dash");
  const rings = el("div", "rings");
  rings.append(ring("cpu", "İşlemci", "var(--cpu)"), ring("gpu", "Ekran kartı", "var(--gpu)"), ring("ram", "Bellek", "var(--ram)"), ring("disk", "Disk", "var(--net)"));
  const deck = el("div", "deck");
  (S.cfg?.shortcuts || []).slice(0, 12).forEach((s, i) => deck.append(keyButton(s, i)));
  p.append(clockBox(), rings, mediaCard(false), deck);
  return p;
}

function sysPage() {
  const p = el("section", "page p-sys");
  const cpu = el("div", "card chart c-cpu");
  cpu.style.setProperty("--c", "var(--cpu)");
  cpu.innerHTML = `<div class="hd"><span class="lbl">İşlemci</span><span class="note">${esc(S.cfg?.cpu_name || "")}</span></div><div class="big"><span><span class="v">–</span><small>%</small></span><span class="chip"></span></div><canvas></canvas>`;
  const gpu = el("div", "card chart c-gpu");
  gpu.style.setProperty("--c", "var(--gpu)");
  gpu.innerHTML = `<div class="hd"><span class="lbl">Ekran kartı</span><span class="note">${esc(S.cfg?.gpu_name || "")}</span></div><div class="big"><span><span class="v">–</span><small>%</small></span></div><canvas></canvas>`;
  const mem = el("div", "card mem");
  mem.style.setProperty("--c", "var(--ram)");
  mem.innerHTML = `<div class="hd"><span class="lbl">Bellek</span><span class="note up"></span></div>
    <div class="meter m-ramb" style="--c: var(--ram)"><div class="row"><span>RAM</span><b class="t">–</b></div><div class="b"><i></i></div></div>
    <div class="disks"></div>`;
  const net = el("div", "card chart c-net");
  net.style.setProperty("--c", "var(--net)");
  net.innerHTML = `<div class="hd"><span class="lbl">Ağ</span><span class="note ping"></span></div><div class="netv"><span class="d">↓ –</span><span class="u">↑ –</span></div><canvas></canvas>`;
  p.append(cpu, gpu, mem, net);
  return p;
}

function statusPage() {
  const p = el("section", "page p-status");
  p.append(ring("q1", "Claude · 5 saat", "#e07a5f"), ring("q2", "Claude · Haftalık", "#e07a5f"), ring("phone", "Telefon", "var(--ok)"), ring("pcbat", "Disk", "var(--net)"));
  const procs = el("div", "card procs");
  procs.style.setProperty("--c", "#f472b6");
  procs.innerHTML = `<div class="hd"><span class="lbl">En çok işlemci</span><span class="note pcount"></span></div><div class="plist"></div>`;
  const vb = el("div", "card vbox");
  vb.innerHTML = `<span class="lbl">Ses</span><span class="pct">–</span><div class="vslider"><i></i></div>
    <div class="vbtns"><button class="press mutebtn" aria-label="Sessize al">${ICON.vol}</button><button class="press vdn" aria-label="Azalt">−</button><button class="press vup" aria-label="Artır">+</button></div>`;
  const sl = vb.querySelector(".vslider");
  const setFrom = (e) => {
    const r = sl.getBoundingClientRect();
    const v = Math.round(Math.max(0, Math.min(1, (r.bottom - e.clientY) / r.height)) * 100);
    S.vol.v = v;
    renderVol();
    send({ t: "vol", v });
  };
  sl.addEventListener("pointerdown", (e) => { sl.setPointerCapture(e.pointerId); setFrom(e); });
  sl.addEventListener("pointermove", (e) => { if (e.buttons) setFrom(e); });
  vb.querySelector(".vdn").onclick = () => { S.vol.v = Math.max(0, S.vol.v - 5); renderVol(); send({ t: "vol", v: S.vol.v }); };
  vb.querySelector(".vup").onclick = () => { S.vol.v = Math.min(100, S.vol.v + 5); renderVol(); send({ t: "vol", v: S.vol.v }); };
  wireVolume(vb);
  const sess = el("div", "card mem sess");
  sess.innerHTML = `<div class="hd"><span class="lbl" style="--c: var(--up)">Bilgisayar</span></div>
    <div class="meter" style="--c: var(--cpu)"><div class="row"><span>İşlemci</span><b class="s-cpu">–</b></div><div class="b"><i class="b-cpu"></i></div></div>
    <div class="meter" style="--c: var(--ram)"><div class="row"><span>Bellek</span><b class="s-ram">–</b></div><div class="b"><i class="b-ram"></i></div></div>
    <div class="muted s-up" style="font-size: 12.5px"></div>`;
  p.append(procs, vb, sess);
  return p;
}

function ambientPage() {
  const p = el("section", "page p-amb");
  const an = el("div", "analog");
  let ticks = "";
  for (let i = 0; i < 60; i++) {
    const big = i % 5 === 0;
    ticks += `<line x1="100" y1="${big ? 14 : 16}" x2="100" y2="${big ? 26 : 21}" stroke="${big ? "var(--text-2)" : "var(--text-3)"}" stroke-width="${big ? 3 : 1.4}" stroke-linecap="round" transform="rotate(${i * 6} 100 100)"/>`;
  }
  const nums = [12, 3, 6, 9].map((n, i) => {
    const a = (i * 90 - 90) * Math.PI / 180;
    return `<text x="${100 + Math.cos(a) * 62}" y="${100 + Math.sin(a) * 62 + 7}" text-anchor="middle" font-size="19" fill="var(--text-2)" font-weight="500">${n}</text>`;
  }).join("");
  an.innerHTML = `<svg viewBox="0 0 200 200"><circle cx="100" cy="100" r="96" fill="var(--card)" stroke="var(--line)" stroke-width="2"/>${ticks}${nums}
    <line class="hh" x1="100" y1="100" x2="100" y2="56" stroke="var(--text)" stroke-width="6" stroke-linecap="round"/>
    <line class="mh" x1="100" y1="100" x2="100" y2="30" stroke="var(--text)" stroke-width="4" stroke-linecap="round"/>
    <line class="sh" x1="100" y1="114" x2="100" y2="24" stroke="var(--accent)" stroke-width="1.8" stroke-linecap="round"/>
    <circle cx="100" cy="100" r="6" fill="var(--accent)"/></svg>`;
  const sun = el("div", "card sun");
  sun.style.setProperty("--c", "#fbbf24");
  sun.innerHTML = `<div class="hd"><span class="lbl">Güneş ve Ay</span><span class="note dl"></span></div>
    <svg viewBox="0 0 200 90" preserveAspectRatio="xMidYMid meet"><line x1="6" y1="80" x2="194" y2="80" stroke="var(--line)" stroke-width="1.5"/>
    <path class="arc" d="M 14 80 A 86 70 0 0 1 186 80" fill="none" stroke="#fbbf24" stroke-width="2.5" stroke-linecap="round" opacity=".9"/>
    <circle class="sd" cx="14" cy="80" r="7" fill="#fbbf24"/></svg>
    <div class="times"><span class="rise">↑ –</span><span class="state muted"></span><span class="set">– ↓</span></div><div class="moon"></div>`;
  const aq = el("div", "card aq");
  aq.style.setProperty("--c", "var(--gpu)");
  aq.innerHTML = `<div class="hd"><span class="lbl">Hava kalitesi</span><span class="note city"></span></div><div class="mid"><span class="n">–</span><span class="muted q"></span></div><div class="chips"></div>`;
  const qt = el("div", "card quote");
  const q = QUOTES[Math.floor((Date.now() / 86400000)) % QUOTES.length];
  qt.innerHTML = `<p>${esc(q[0])}</p><small>— ${esc(q[1])}</small>`;
  const fc = el("div", "card fc");
  fc.innerHTML = `<div class="hd"><span class="lbl">7 günlük tahmin</span><span class="note city"></span></div><div class="days"></div>`;
  p.append(an, sun, qt, aq, fc);
  return p;
}

function mediaPage() {
  const p = el("section", "page p-media");
  const c = el("div", "cover");
  c.innerHTML = "<span>♪</span>";
  const m = mediaCard(true);
  m.querySelector(".cover").remove();
  p.append(c, m);
  return p;
}

function keysPage() {
  const p = el("section", "page p-keys");
  const deck = el("div", "deck");
  (S.cfg?.shortcuts || []).forEach((s, i) => deck.append(keyButton(s, i)));
  p.append(deck);
  return p;
}

function clockPage() {
  const p = el("section", "page p-clock");
  p.append(clockBox());
  return p;
}

const QUOTES = [
  ["Damlaya damlaya göl olur.", "Atasözü"],
  ["Sakla samanı, gelir zamanı.", "Atasözü"],
  ["Bin işit, bir söyle.", "Atasözü"],
  ["Güneş balçıkla sıvanmaz.", "Atasözü"],
  ["Ağaç yaşken eğilir.", "Atasözü"],
  ["Emek olmadan yemek olmaz.", "Atasözü"],
  ["Söz gümüşse sükût altındır.", "Atasözü"],
  ["Bir elin nesi var, iki elin sesi var.", "Atasözü"],
  ["Acele işe şeytan karışır.", "Atasözü"],
  ["Gülü seven dikenine katlanır.", "Atasözü"],
  ["Az olsun, öz olsun.", "Atasözü"],
  ["Her işte bir hayır vardır.", "Atasözü"],
  ["Bin yıllık yolculuk tek bir adımla başlar.", "Lao Tzu"],
  ["Bildiğim tek şey, hiçbir şey bilmediğimdir.", "Sokrates"],
];

// ---------------------------------------------------------------------------
// Çizim
// ---------------------------------------------------------------------------
function renderAll() {
  renderClock();
  renderWeather();
  renderStats();
  renderSys();
  renderRings();
  renderMedia();
  renderVol();
}

const DAYS = ["Pazar", "Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi"];
const DAYS_S = ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"];
const MONTHS = ["Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık"];
const p2 = (n) => String(n).padStart(2, "0");

function renderClock() {
  const d = new Date();
  for (const c of $$(".clockbox")) {
    c.querySelector(".hh").textContent = p2(d.getHours());
    c.querySelector(".mm").textContent = p2(d.getMinutes());
    c.querySelector(".ss").textContent = p2(d.getSeconds());
    c.querySelector(".date").textContent = `${d.getDate()} ${MONTHS[d.getMonth()]} ${DAYS[d.getDay()]}`;
    c.querySelector(".pc").textContent = `${(S.cfg?.pc || "").toUpperCase()}${location.hostname === "127.0.0.1" ? " · USB" : " · Wi-Fi"}`;
    const ph = phoneBattery();
    c.querySelector(".bat").textContent = ph ? `· 📱 %${ph.level}${ph.charging ? " ⚡" : ""}` : "";
  }
  // Analog saat
  for (const a of $$(".analog svg")) {
    const s = d.getSeconds(), m = d.getMinutes() + s / 60, h = (d.getHours() % 12) + m / 60;
    a.querySelector(".hh").setAttribute("transform", `rotate(${h * 30} 100 100)`);
    a.querySelector(".mh").setAttribute("transform", `rotate(${m * 6} 100 100)`);
    a.querySelector(".sh").setAttribute("transform", `rotate(${s * 6} 100 100)`);
  }
}
setInterval(() => { renderClock(); tickProgress(); }, 1000);

function phoneBattery() {
  try {
    const j = Native?.battery?.();
    if (j) return JSON.parse(j);
  } catch {}
  return null;
}

const fmtRate = (b) => {
  if (b >= 1048576) return [(b / 1048576).toFixed(1), "MB/s"];
  if (b >= 1024) return [(b / 1024).toFixed(0), "KB/s"];
  return [Math.round(b), "B/s"];
};
const fmtUp = (s) => {
  const d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60);
  return d ? `${d} g ${h} sa` : h ? `${h} sa ${m} dk` : `${m} dk`;
};

function renderStats() {
  const s = S.stats;
  if (!s) return;
  const col = { cpu: css("--cpu"), gpu: css("--gpu"), ram: css("--ram"), net: css("--net"), up: css("--up") };
  const [dv, du] = fmtRate(s.down);
  const [uv, uu] = fmtRate(s.up);
  for (const t of $$(".tile")) {
    const val = t.querySelector(".val"), sub = t.querySelector(".sub"), cv = t.querySelector("canvas");
    if (t.classList.contains("m-cpu")) { val.innerHTML = `${Math.round(s.cpu)}<small>%</small>`; sub.textContent = S.cfg?.cores ? `${S.cfg.cores} çekirdek` : ""; chart(cv, [[S.hist.cpu, col.cpu]], 100); }
    else if (t.classList.contains("m-gpu")) { val.innerHTML = `${Math.round(s.gpu)}<small>%</small>`; sub.textContent = s.gpu < 3 ? "Boşta" : "Çalışıyor"; chart(cv, [[S.hist.gpu, col.gpu]], 100); }
    else if (t.classList.contains("m-ram")) { val.innerHTML = `${(s.ram_used || 0).toFixed(1)}<small>GB</small>`; sub.textContent = `/ ${(s.ram_total || 0).toFixed(0)} GB`; chart(cv, [[S.hist.ram, col.ram]], 100); }
    else {
      val.innerHTML = `${dv}<small>${du}</small>`;
      sub.textContent = `↑ ${uv} ${uu}${S.sys?.ping != null ? ` · ${Math.round(S.sys.ping)} ms` : ""}`;
      chart(cv, [[S.hist.down, col.net]], Math.max(1, ...S.hist.down) * 1.15);
    }
  }
  // Sistem sayfası
  for (const c of $$(".c-cpu")) { c.querySelector(".v").textContent = Math.round(s.cpu); c.querySelector(".chip").textContent = S.cfg?.cores ? `${S.cfg.cores} çekirdek` : ""; chart(c.querySelector("canvas"), [[S.hist.cpu, col.cpu]], 100, true); }
  for (const c of $$(".c-gpu")) { c.querySelector(".v").textContent = Math.round(s.gpu); chart(c.querySelector("canvas"), [[S.hist.gpu, col.gpu]], 100, true); }
  for (const c of $$(".c-net")) {
    c.querySelector(".d").textContent = `↓ ${dv} ${du}`;
    c.querySelector(".u").textContent = `↑ ${uv} ${uu}`;
    chart(c.querySelector("canvas"), [[S.hist.down, col.net], [S.hist.up, col.up]], Math.max(1, ...S.hist.down, ...S.hist.up) * 1.15, true);
  }
  for (const m of $$(".m-ramb")) {
    m.querySelector(".t").textContent = `${(s.ram_used || 0).toFixed(1)} / ${(s.ram_total || 0).toFixed(0)} GB`;
    m.querySelector(".b i").style.width = `${s.ram}%`;
  }
  for (const c of $$(".sess")) {
    c.querySelector(".s-cpu").textContent = `%${Math.round(s.cpu)}`;
    c.querySelector(".b-cpu").style.width = `${s.cpu}%`;
    c.querySelector(".s-ram").textContent = `${(s.ram_used || 0).toFixed(1)} GB`;
    c.querySelector(".b-ram").style.width = `${s.ram}%`;
  }
  // Halkalar
  for (const r of $$(".r-cpu")) setRing(r, s.cpu, Math.round(s.cpu), "%", S.cfg?.cores ? `${S.cfg.cores} çekirdek` : "");
  for (const r of $$(".r-gpu")) setRing(r, s.gpu, Math.round(s.gpu), "%", s.gpu < 3 ? "Boşta" : "");
  for (const r of $$(".r-ram")) setRing(r, s.ram, (s.ram_used || 0).toFixed(1), "GB", `/ ${(s.ram_total || 0).toFixed(0)} GB`);
  for (const r of $$(".r-disk")) setRing(r, s.disk || 0, Math.round(s.disk || 0), "%", "Sistem diski");
  renderRings();
}

function renderSys() {
  const y = S.sys;
  if (!y) return;
  for (const n of $$(".p-sys .up")) n.textContent = y.uptime ? `Açık ${fmtUp(y.uptime)}` : "";
  for (const n of $$(".s-up")) n.textContent = y.uptime ? `Açık kalma: ${fmtUp(y.uptime)}` : "";
  for (const n of $$(".ping")) n.textContent = y.ping != null ? `Ping ${Math.round(y.ping)} ms` : "";
  for (const box of $$(".disks")) {
    box.replaceChildren(...(y.disks || []).slice(0, 4).map((d) => {
      const pct = d.total ? (d.used / d.total) * 100 : 0;
      const m = el("div", "meter", `<div class="row"><span>${esc(d.name)}</span><b>%${Math.round(pct)}</b></div><div class="b"><i></i></div>`);
      m.style.setProperty("--c", pct > 90 ? "var(--err)" : "var(--text-2)");
      requestAnimationFrame(() => (m.querySelector("i").style.width = `${pct}%`));
      m.title = `${d.used.toFixed(0)} / ${d.total.toFixed(0)} GB`;
      return m;
    }));
  }
  for (const box of $$(".plist")) {
    const rows = (y.procs || []).slice(0, 6).map((p, i) => {
      const r = el("button", "prow");
      const ic = S.icons[p.exe];
      const hue = (p.name.charCodeAt(0) * 37) % 360;
      r.innerHTML = `<span class="pi" style="background:${ic ? "transparent" : `hsl(${hue} 70% 65%)`}">${ic ? `<img src="${ic}" alt="">` : esc(p.name[0]?.toUpperCase() || "?")}</span>
        <span class="pn">${esc(p.name)}${p.count > 1 ? `<small>×${p.count}</small>` : ""}</span><span class="pc">%${p.cpu.toFixed(1)}</span>`;
      r.onclick = () => askKill(p);
      return r;
    });
    box.replaceChildren(...rows);
  }
  for (const n of $$(".pcount")) n.textContent = y.procs?.length ? `${y.total_procs || ""} süreç` : "";
}

function askKill(p) {
  const sh = $("#sheet");
  sh.replaceChildren();
  const card = el("div", "sheet-card");
  card.style.maxWidth = "420px";
  card.innerHTML = `<h2>${esc(p.name)}${p.count > 1 ? ` · ${p.count} örnek` : ""}</h2>
    <p class="muted" style="margin:0 0 14px">İşlemci %${p.cpu.toFixed(1)} · Bellek ${p.mem.toFixed(1)} GB. Kaydedilmemiş işler kaybolabilir.</p>`;
  const row = el("div", "themes");
  const one = el("button", "press", "En yoğun olanı kapat");
  const all = el("button", "press", `Hepsini kapat${p.count > 1 ? ` (${p.count})` : ""}`);
  all.style.color = "var(--err)";
  const no = el("button", "press", "Vazgeç");
  one.onclick = () => { send({ t: "kill", name: p.name, all: false }); closeSheet(); toast(`${p.name} kapatılıyor`); };
  all.onclick = () => { send({ t: "kill", name: p.name, all: true }); closeSheet(); toast(`${p.name} kapatılıyor`); };
  no.onclick = closeSheet;
  row.append(one, all, no);
  card.append(row);
  sh.append(card);
  sh.hidden = false;
  sh.onclick = (e) => { if (e.target === sh) closeSheet(); };
}

function renderRings() {
  const q = S.claude?.limits || [];
  const fmtReset = (iso) => {
    const ms = Date.parse(iso) - Date.now();
    if (!(ms > 0)) return "";
    const h = Math.floor(ms / 3600000), m = Math.floor((ms % 3600000) / 60000);
    return h >= 24 ? `${Math.floor(h / 24)} g ${h % 24} sa sonra sıfırlanır` : `${h} sa ${m} dk sonra sıfırlanır`;
  };
  const slots = [$$(".r-q1"), $$(".r-q2")];
  slots.forEach((rs, i) => rs.forEach((r) => {
    const l = q[i];
    if (l) {
      r.querySelector(".lbl").textContent = `Claude · ${l.label}`;
      setRing(r, l.percent, Math.round(l.percent), "%", fmtReset(l.resets_at));
    } else {
      // Claude yoksa: işlemci ve bellek
      const s = S.stats;
      if (!s) return;
      r.style.setProperty("--c", i ? "var(--ram)" : "var(--cpu)");
      r.querySelector(".lbl").textContent = i ? "Bellek" : "İşlemci";
      if (i) setRing(r, s.ram, Math.round(s.ram), "%", `${(s.ram_used || 0).toFixed(1)} / ${(s.ram_total || 0).toFixed(0)} GB`);
      else setRing(r, s.cpu, Math.round(s.cpu), "%", S.cfg?.cores ? `${S.cfg.cores} çekirdek` : "");
    }
  }));
  const ph = phoneBattery();
  for (const r of $$(".r-phone")) {
    if (ph) setRing(r, ph.level, ph.level, "%", ph.charging ? `⚡ Şarj oluyor${ph.temp ? ` · ${ph.temp}°C` : ""}` : ph.temp ? `${ph.temp}°C` : "Pilde");
    else setRing(r, 0, "–", "", "Telefon pili okunamadı");
  }
  const s = S.stats;
  for (const r of $$(".r-pcbat")) {
    if (!s) continue;
    if (s.battery >= 0) { r.querySelector(".lbl").textContent = "Bilgisayar pili"; setRing(r, s.battery, s.battery, "%", s.charging ? "⚡ Şarj oluyor" : "Pilde"); }
    else setRing(r, s.disk || 0, Math.round(s.disk || 0), "%", "Sistem diski dolu");
  }
}

function chart(cv, series, max, grid) {
  if (!cv) return;
  const dpr = window.devicePixelRatio || 1;
  const w = cv.clientWidth, h = cv.clientHeight;
  if (!w || !h) return;
  if (cv.width !== Math.round(w * dpr) || cv.height !== Math.round(h * dpr)) { cv.width = Math.round(w * dpr); cv.height = Math.round(h * dpr); }
  const g = cv.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  if (grid) {
    g.strokeStyle = css("--line");
    g.lineWidth = 1;
    for (let k = 1; k < 4; k++) { const y = Math.round((h * k) / 4) + .5; g.beginPath(); g.moveTo(0, y); g.lineTo(w, y); g.stroke(); }
  }
  const glow = Number(css("--glow")) || 1;
  for (const [data, col] of series) {
    if (data.length < 2) continue;
    const step = w / (HMAX - 1);
    const off = (HMAX - data.length) * step;
    const y = (v) => h - 2 - (Math.min(max, v) / max) * (h - 10);
    g.beginPath();
    data.forEach((v, i) => (i ? g.lineTo(off + i * step, y(v)) : g.moveTo(off, y(v))));
    g.lineWidth = 2.2; g.strokeStyle = col; g.lineJoin = "round";
    g.shadowColor = col; g.shadowBlur = 8 * glow;
    g.stroke();
    g.shadowBlur = 0;
    g.lineTo(off + (data.length - 1) * step, h); g.lineTo(off, h); g.closePath();
    const grad = g.createLinearGradient(0, 0, 0, h);
    grad.addColorStop(0, hexA(col, .3)); grad.addColorStop(1, hexA(col, 0));
    g.fillStyle = grad; g.fill();
  }
}
function hexA(c, a) {
  const m = /^#([0-9a-f]{6})$/i.exec(c.trim());
  if (!m) return c;
  const n = parseInt(m[1], 16);
  return `rgba(${n >> 16},${(n >> 8) & 255},${n & 255},${a})`;
}

const mmss = (ms) => {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${p2(s % 60)}`;
};

function renderMedia() {
  const m = S.media;
  const has = !!m.title;
  const src = (m.source || "").replace(/\.exe.*$/i, "").replace(/^.*[\\!]/, "").replace(/^Spotify.*/i, "Spotify");
  for (const b of $$(".mcard")) {
    b.querySelector(".mtitle").textContent = has ? m.title : "Çalan bir şey yok";
    b.querySelector(".martist").textContent = has ? [m.artist, m.album].filter(Boolean).join(" · ") : "Bilgisayarda bir şey çalınca burada görünür";
    b.querySelector(".src").textContent = has && src ? src : "Medya";
    b.querySelector('[data-a="toggle"]').innerHTML = m.playing ? ICON.pause : ICON.play;
  }
  for (const cv of $$(".cover")) {
    const want = m.cover || "";
    if (cv.dataset.src !== want) {
      cv.dataset.src = want;
      cv.innerHTML = want ? `<img src="${want}" alt="">` : "<span>♪</span>";
    }
  }
  tickProgress();
}

function tickProgress() {
  const m = S.media;
  const pos = m.playing ? m.pos + (Date.now() - m.at) : m.pos;
  const dur = m.dur || 0;
  for (const b of $$(".mcard")) {
    b.querySelector(".cur").textContent = mmss(Math.min(pos, dur || pos));
    b.querySelector(".dur").textContent = mmss(dur);
    b.querySelector(".bar i").style.width = dur ? `${Math.min(100, (pos / dur) * 100)}%` : "0";
  }
}

function renderVol(except) {
  for (const r of $$(".volr")) {
    if (r !== except && document.activeElement !== r) r.value = Math.round(S.vol.v);
    r.style.setProperty("--p", `${r.value}%`);
  }
  for (const n of $$(".vol .n")) n.textContent = Math.round(S.vol.v);
  for (const b of $$(".mutebtn")) b.innerHTML = S.vol.m ? ICON.mute : ICON.vol;
  for (const v of $$(".vbox")) {
    v.querySelector(".pct").textContent = S.vol.m ? "Sessiz" : `%${Math.round(S.vol.v)}`;
    v.querySelector(".vslider i").style.height = `${S.vol.m ? 0 : S.vol.v}%`;
  }
}

// ---------------------------------------------------------------------------
// Hava durumu, gök, hava kalitesi (telefon doğrudan Open-Meteo'dan alır)
// ---------------------------------------------------------------------------
const WMO = {
  0: ["Açık", "☀️", "🌙"], 1: ["Az bulutlu", "🌤️", "🌙"], 2: ["Parçalı bulutlu", "⛅", "☁️"], 3: ["Bulutlu", "☁️", "☁️"],
  45: ["Sisli", "🌫️"], 48: ["Kırağılı sis", "🌫️"], 51: ["Hafif çisenti", "🌦️"], 53: ["Çisenti", "🌦️"], 55: ["Yoğun çisenti", "🌧️"],
  61: ["Hafif yağmur", "🌦️"], 63: ["Yağmurlu", "🌧️"], 65: ["Şiddetli yağmur", "🌧️"], 66: ["Donan yağmur", "🌧️"], 67: ["Donan yağmur", "🌧️"],
  71: ["Hafif kar", "🌨️"], 73: ["Karlı", "🌨️"], 75: ["Yoğun kar", "❄️"], 77: ["Kar taneleri", "🌨️"],
  80: ["Sağanak", "🌦️"], 81: ["Sağanak", "🌧️"], 82: ["Şiddetli sağanak", "⛈️"], 85: ["Kar sağanağı", "🌨️"], 86: ["Kar sağanağı", "❄️"],
  95: ["Gök gürültülü", "⛈️"], 96: ["Dolu", "⛈️"], 99: ["Dolu", "⛈️"],
};
let wxTimer = null;
async function loadWeather() {
  clearTimeout(wxTimer);
  wxTimer = setTimeout(loadWeather, 15 * 60 * 1000);
  const w = S.cfg?.weather;
  if (!w || w.enabled === false || w.lat == null) return;
  try {
    const url = `https://api.open-meteo.com/v1/forecast?latitude=${w.lat}&longitude=${w.lon}&current=temperature_2m,apparent_temperature,weather_code,is_day&daily=weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,uv_index_max,daylight_duration&forecast_days=7&timezone=auto`;
    const r = await (await fetch(url)).json();
    S.wx = { city: w.city, ...r.current, daily: r.daily };
    renderWeather();
  } catch {}
  try {
    const url = `https://air-quality-api.open-meteo.com/v1/air-quality?latitude=${w.lat}&longitude=${w.lon}&current=european_aqi,pm2_5,pm10,ozone,nitrogen_dioxide`;
    S.aq = (await (await fetch(url)).json()).current;
    renderWeather();
  } catch {}
}

function moonPhase(d = new Date()) {
  const lp = 2551443;
  const ref = Date.UTC(1970, 0, 7, 20, 35) / 1000;
  const ph = (((d.getTime() / 1000 - ref) % lp) + lp) % lp / lp;
  const names = [["Yeni ay", "🌑"], ["Hilal (büyüyen)", "🌒"], ["İlk dördün", "🌓"], ["Şişkin ay (büyüyen)", "🌔"], ["Dolunay", "🌕"], ["Şişkin ay (küçülen)", "🌖"], ["Son dördün", "🌗"], ["Hilal (küçülen)", "🌘"]];
  const [n, e] = names[Math.round(ph * 8) % 8];
  const lit = Math.round((1 - Math.cos(ph * 2 * Math.PI)) / 2 * 100);
  return { name: n, emoji: e, lit };
}

function renderWeather() {
  const w = S.wx;
  for (const box of $$(".wx")) {
    if (!w) { box.innerHTML = ""; continue; }
    const [desc, day, night] = WMO[w.weather_code] || ["", "🌡️"];
    const ic = w.is_day === 0 && night ? night : day;
    const dmax = w.daily?.temperature_2m_max?.[0], dmin = w.daily?.temperature_2m_min?.[0];
    box.innerHTML = `<span class="ic">${ic}</span><span class="tmp">${Math.round(w.temperature_2m)}°</span>
      <span class="desc"><b>${esc(desc)}${w.city ? " · " + esc(w.city) : ""}</b>↑${Math.round(dmax)}° ↓${Math.round(dmin)}° · Hissedilen ${Math.round(w.apparent_temperature)}°</span>`;
  }
  for (const n of $$(".city")) n.textContent = w?.city || "";
  // 7 gün
  for (const box of $$(".fc .days")) {
    if (!w?.daily) continue;
    box.replaceChildren(...w.daily.time.slice(0, 7).map((t, i) => {
      const d = new Date(t + "T12:00");
      const [, ic] = WMO[w.daily.weather_code[i]] || ["", "🌡️"];
      return el("div", "", `<span>${i === 0 ? "Bugün" : DAYS_S[d.getDay()]}</span><span class="ic">${ic}</span><span><b>${Math.round(w.daily.temperature_2m_max[i])}°</b> ${Math.round(w.daily.temperature_2m_min[i])}°</span>`);
    }));
  }
  // Güneş
  for (const s of $$(".sun")) {
    if (!w?.daily) continue;
    const rise = new Date(w.daily.sunrise[0]), set = new Date(w.daily.sunset[0]);
    const now = Date.now();
    const f = Math.max(0, Math.min(1, (now - rise) / (set - rise)));
    const a = Math.PI * (1 - f);
    const cx = 100 + Math.cos(a) * 86, cy = 80 - Math.sin(a) * 70;
    const dot = s.querySelector(".sd");
    dot.setAttribute("cx", cx.toFixed(1)); dot.setAttribute("cy", cy.toFixed(1));
    const up = now > rise && now < set;
    dot.setAttribute("fill", up ? "#fbbf24" : "var(--text-3)");
    s.querySelector(".rise").textContent = `↑ ${p2(rise.getHours())}:${p2(rise.getMinutes())}`;
    s.querySelector(".set").textContent = `${p2(set.getHours())}:${p2(set.getMinutes())} ↓`;
    s.querySelector(".state").textContent = now < rise ? "gün doğmadan" : now > set ? "gün batımından sonra" : `${Math.round((set - now) / 60000 / 60 * 10) / 10} sa gün ışığı`;
    const dl = w.daily.daylight_duration?.[0];
    s.querySelector(".dl").textContent = `${dl ? `Gün ${Math.floor(dl / 3600)} sa ${Math.round((dl % 3600) / 60)} dk` : ""}${w.daily.uv_index_max?.[0] != null ? ` · UV ${Math.round(w.daily.uv_index_max[0])}` : ""}`;
    const m = moonPhase();
    s.querySelector(".moon").textContent = `${m.emoji} ${m.name} · %${m.lit}`;
  }
  // Hava kalitesi
  for (const b of $$(".aq")) {
    const q = S.aq;
    if (!q) continue;
    const v = Math.round(q.european_aqi);
    const [name, col] = v <= 20 ? ["Çok iyi", "#22c55e"] : v <= 40 ? ["İyi", "#84cc16"] : v <= 60 ? ["Orta", "#eab308"] : v <= 80 ? ["Kötü", "#f97316"] : v <= 100 ? ["Çok kötü", "#ef4444"] : ["Son derece kötü", "#a855f7"];
    b.querySelector(".n").textContent = v;
    b.querySelector(".n").style.color = col;
    b.querySelector(".q").textContent = `${name} · Avrupa HKİ`;
    b.querySelector(".chips").innerHTML = [["PM2.5", q.pm2_5], ["PM10", q.pm10], ["O₃", q.ozone], ["NO₂", q.nitrogen_dioxide]]
      .filter((x) => x[1] != null).map(([k, x]) => `<span class="chip">${k} ${Math.round(x)}</span>`).join("");
  }
}
setInterval(renderWeather, 60_000);

// ---------------------------------------------------------------------------
// Başlangıç
// ---------------------------------------------------------------------------
window.addEventListener("resize", () => { renderStats(); goTo(currentIndex(), false); });
showConn(true);
if (location.protocol.startsWith("http")) connect();
