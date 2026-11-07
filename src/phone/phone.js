// Cam Dock telefon ekranı. Cam Dock'un içindeki sunucudan gelir; veriler
// WebSocket ile akar. Bağlantı koparsa kendiliğinden yeniden dener.
"use strict";

const $ = (s, r = document) => r.querySelector(s);
const el = (tag, cls, html) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (html != null) e.innerHTML = html;
  return e;
};
const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const Native = window.Android || null;

const ICON = {
  prev: '<svg viewBox="0 0 24 24"><path d="M6 5h2v14H6zM20 5v14L9.5 12z"/></svg>',
  next: '<svg viewBox="0 0 24 24"><path d="M16 5h2v14h-2zM4 5v14l10.5-7z"/></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="M7 4.5v15l13-7.5z"/></svg>',
  pause: '<svg viewBox="0 0 24 24"><path d="M6 4h4.5v16H6zM13.5 4H18v16h-4.5z"/></svg>',
  vol: '<svg viewBox="0 0 24 24"><path d="M3 9v6h4l5 4V5L7 9zm13.5 3a4.5 4.5 0 0 0-2.5-4v8a4.5 4.5 0 0 0 2.5-4z"/></svg>',
  mute: '<svg viewBox="0 0 24 24"><path d="M3 9v6h4l5 4V5L7 9zm16.6 3 2.1-2.1-1.4-1.4-2.1 2.1-2.1-2.1-1.4 1.4 2.1 2.1-2.1 2.1 1.4 1.4 2.1-2.1 2.1 2.1 1.4-1.4z"/></svg>',
};

// ---------------------------------------------------------------------------
// Durum
// ---------------------------------------------------------------------------
const S = {
  cfg: null,
  stats: null,
  hist: { cpu: [], gpu: [], ram: [], down: [] },
  media: { title: "", artist: "", playing: false, pos: 0, dur: 0, cover: null, at: Date.now() },
  vol: { v: 50, m: false },
  wx: null,
  authed: false,
};
const HMAX = 60;

// Anahtar: adres çubuğundan gelirse (USB'de Cam Dock verir) sakla
(() => {
  const u = new URL(location.href);
  const t = u.searchParams.get("token");
  if (t) {
    try { localStorage.setItem("camdock-token", t); } catch {}
    history.replaceState(null, "", "/");
  }
})();
const token = () => { try { return localStorage.getItem("camdock-token") || ""; } catch { return ""; } };
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
  const proto = location.protocol === "https:" ? "wss" : "ws";
  ws = new WebSocket(`${proto}://${location.host}/ws`);
  ws.onopen = () => {
    fails = 0;
    send({ t: "hello", token: token(), device: deviceName() });
  };
  ws.onmessage = (e) => {
    let m;
    try { m = JSON.parse(e.data); } catch { return; }
    onMsg(m);
  };
  ws.onclose = () => {
    S.authed = false;
    showConn(true);
    fails++;
    // USB kablosu çıkarsa uygulama Wi-Fi adresine geçebilir
    if (fails >= 3) { try { Native?.connectionLost?.(); } catch {} }
    retryTimer = setTimeout(connect, Math.min(4000, 600 + fails * 500));
  };
  ws.onerror = () => {};
}

function send(o) {
  if (ws && ws.readyState === 1) ws.send(JSON.stringify(o));
}

function showConn(on, text = "Bilgisayara bağlanıyor…") {
  const c = $("#conn");
  c.querySelector("span").textContent = text;
  c.classList.toggle("show", on);
}

function onMsg(m) {
  switch (m.t) {
    case "auth":
      if (m.ok) {
        S.authed = true;
        if (m.token) { try { localStorage.setItem("camdock-token", m.token); } catch {} }
        $("#pair").hidden = true;
        showConn(false);
        try { Native?.connected?.(); } catch {}
      } else {
        showConn(false);
        showPair(m.pc, m.wrong);
      }
      break;
    case "hist":
      for (const h of m.items || []) {
        push(S.hist.cpu, h.cpu); push(S.hist.gpu, h.gpu); push(S.hist.ram, h.ram);
      }
      break;
    case "cfg": applyCfg(m); break;
    case "stats":
      S.stats = m;
      push(S.hist.cpu, m.cpu); push(S.hist.gpu, m.gpu); push(S.hist.ram, m.ram); push(S.hist.down, m.down);
      renderStats();
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
  }
}

function push(a, v) {
  a.push(Number(v) || 0);
  if (a.length > HMAX) a.shift();
}

// ---------------------------------------------------------------------------
// Eşleşme
// ---------------------------------------------------------------------------
function showPair(pc, wrong) {
  const p = $("#pair");
  p.hidden = false;
  $("#pair-pc").textContent = pc || "Cam Dock";
  const err = $("#pair-err");
  err.textContent = wrong ? "Kod yanlış, yeniden dene." : "";
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
// Sayfalar
// ---------------------------------------------------------------------------
const PAGES = { home: homePage, media: mediaPage, shortcuts: shortcutsPage, stats: statsPage };
let pagesKey = "";

function applyCfg(c) {
  const first = !S.cfg;
  S.cfg = c;
  document.documentElement.style.setProperty("--accent", c.accent || "#2f7bff");
  document.documentElement.classList.toggle("light", c.dark === false);
  const key = JSON.stringify([c.pages, c.shortcuts]);
  if (key !== pagesKey) {
    pagesKey = key;
    buildPages();
  }
  if (first || !S.wx) loadWeather();
}

function buildPages() {
  const root = $("#pages");
  const keep = root.scrollLeft / Math.max(1, root.clientWidth);
  root.replaceChildren();
  const list = (S.cfg.pages || []).filter((p) => PAGES[p]);
  if (!list.length) list.push("home");
  for (const p of list) root.append(PAGES[p]());
  const dots = $("#dots");
  dots.replaceChildren(...list.map(() => el("i")));
  root.scrollLeft = Math.round(keep) * root.clientWidth;
  updateDots();
  renderAll();
}

$("#pages").addEventListener("scroll", () => updateDots(), { passive: true });
function updateDots() {
  const r = $("#pages");
  const i = Math.round(r.scrollLeft / Math.max(1, r.clientWidth));
  [...$("#dots").children].forEach((d, k) => d.classList.toggle("on", k === i));
}

function tile(kind, label, color) {
  const t = el("div", `card tile t-${kind}`);
  t.style.setProperty("--c", color);
  t.innerHTML = `<span class="lbl">${label}</span><span class="val">–</span><span class="sub"></span><canvas></canvas>`;
  return t;
}

function mediaBlock(big) {
  const b = el("div", big ? "page mediap" : "card mini-media");
  b.innerHTML = `
    <div class="cover"><span>♪</span></div>
    <div class="mtext"><span class="lbl">Medya</span><div class="mtitle">Çalan bir şey yok</div><div class="martist"></div>${big ? '<div class="src"></div>' : ""}
      ${big ? `<div class="ctrls">${ctrls()}</div><div class="prog"><span class="cur">0:00</span><div class="bar"><i></i></div><span class="dur">0:00</span></div><div class="vol"><button class="cbtn press mutebtn">${ICON.vol}</button><input type="range" min="0" max="100" class="volr"></div>` : ""}
    </div>
    ${big ? "" : `<div class="ctrls">${ctrls()}<div class="vol"><button class="cbtn press mutebtn">${ICON.vol}</button><input type="range" min="0" max="100" class="volr"></div></div><div class="prog"><span class="cur">0:00</span><div class="bar"><i></i></div><span class="dur">0:00</span></div>`}`;
  b.querySelectorAll("[data-a]").forEach((x) => (x.onclick = () => {
    send({ t: "media", a: x.dataset.a });
    if (x.dataset.a === "toggle") { S.media.playing = !S.media.playing; S.media.at = Date.now(); renderMedia(); }
  }));
  b.querySelectorAll(".volr").forEach((r) => {
    let last = 0;
    r.oninput = () => {
      S.vol.v = Number(r.value);
      renderVol(r);
      const now = Date.now();
      if (now - last > 60) { last = now; send({ t: "vol", v: S.vol.v }); }
    };
    r.onchange = () => send({ t: "vol", v: Number(r.value) });
  });
  b.querySelectorAll(".mutebtn").forEach((x) => (x.onclick = () => { S.vol.m = !S.vol.m; send({ t: "mute", m: S.vol.m }); renderVol(); }));
  return b;
}
const ctrls = () => `<button class="cbtn press" data-a="prev" aria-label="Önceki">${ICON.prev}</button><button class="cbtn main press" data-a="toggle" aria-label="Oynat/Duraklat">${ICON.play}</button><button class="cbtn press" data-a="next" aria-label="Sonraki">${ICON.next}</button>`;

function homePage() {
  const p = el("section", "page home");
  const clock = el("div", "clockbox");
  clock.innerHTML = `
    <div class="topline"><i class="dot"></i><span class="pc"></span><span class="bat"></span></div>
    <div class="time"><span class="hm">--:--</span><sup class="ss">00</sup></div>
    <div class="date"></div>
    <div class="wx"></div>`;
  const tiles = el("div", "tiles");
  tiles.append(tile("cpu", "İşlemci", "var(--cpu)"), tile("gpu", "Ekran kartı", "var(--gpu)"), tile("ram", "Bellek", "var(--ram)"), tile("net", "Ağ", "var(--net)"));
  p.append(clock, mediaBlock(false), tiles);
  return p;
}

function mediaPage() {
  return mediaBlock(true);
}

function shortcutsPage() {
  const p = el("section", "page shortcuts");
  (S.cfg?.shortcuts || []).forEach((s, i) => {
    const b = el("button", "sc press");
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
    p.append(b);
  });
  return p;
}

function statsPage() {
  const p = el("section", "page statsp");
  p.append(tile("cpu", "İşlemci", "var(--cpu)"), tile("gpu", "Ekran kartı", "var(--gpu)"), tile("ram", "Bellek", "var(--ram)"), tile("net", "Ağ", "var(--net)"));
  return p;
}

// ---------------------------------------------------------------------------
// Çizim
// ---------------------------------------------------------------------------
function renderAll() {
  renderClock();
  renderWeather();
  renderStats();
  renderMedia();
  renderVol();
}

const DAYS = ["Pazar", "Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi"];
const MONTHS = ["Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık"];
function renderClock() {
  const d = new Date();
  const p2 = (n) => String(n).padStart(2, "0");
  for (const c of document.querySelectorAll(".clockbox")) {
    c.querySelector(".hm").textContent = `${p2(d.getHours())}:${p2(d.getMinutes())}`;
    c.querySelector(".ss").textContent = p2(d.getSeconds());
    c.querySelector(".date").textContent = `${d.getDate()} ${MONTHS[d.getMonth()]} ${DAYS[d.getDay()]}`;
    c.querySelector(".pc").textContent = (S.cfg?.pc || "").toUpperCase();
    const b = S.stats?.battery ?? -1;
    c.querySelector(".bat").textContent = b >= 0 ? `· 🔋 %${b}${S.stats.charging ? " ⚡" : ""}` : "";
  }
}
setInterval(() => { renderClock(); tickProgress(); }, 1000);

const fmtRate = (b) => {
  if (b >= 1048576) return [(b / 1048576).toFixed(1), "MB/s"];
  if (b >= 1024) return [(b / 1024).toFixed(0), "KB/s"];
  return [Math.round(b), "B/s"];
};

function renderStats() {
  const s = S.stats;
  if (!s) return;
  for (const t of document.querySelectorAll(".tile")) {
    const val = t.querySelector(".val");
    const sub = t.querySelector(".sub");
    let data, max = 100;
    if (t.classList.contains("t-cpu")) { val.innerHTML = `${Math.round(s.cpu)}<small>%</small>`; sub.textContent = ""; data = S.hist.cpu; }
    else if (t.classList.contains("t-gpu")) { val.innerHTML = `${Math.round(s.gpu)}<small>%</small>`; sub.textContent = s.gpu < 3 ? "Boşta" : ""; data = S.hist.gpu; }
    else if (t.classList.contains("t-ram")) { val.innerHTML = `${(s.ram_used || 0).toFixed(1)}<small>GB</small>`; sub.textContent = `/ ${(s.ram_total || 0).toFixed(0)} GB · %${Math.round(s.ram)}`; data = S.hist.ram; }
    else {
      const [v, u] = fmtRate(s.down);
      const [uv, uu] = fmtRate(s.up);
      val.innerHTML = `${v}<small>${u}</small>`;
      sub.textContent = `↑ ${uv} ${uu}`;
      data = S.hist.down;
      max = Math.max(1, ...data) * 1.15;
    }
    spark(t.querySelector("canvas"), data, max, getComputedStyle(t).getPropertyValue("--c").trim() || "#38bdf8");
  }
}

function spark(cv, data, max, color) {
  const dpr = window.devicePixelRatio || 1;
  const w = cv.clientWidth, h = cv.clientHeight;
  if (!w || !h) return;
  if (cv.width !== Math.round(w * dpr)) { cv.width = Math.round(w * dpr); cv.height = Math.round(h * dpr); }
  const g = cv.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  if (data.length < 2) return;
  const col = color.startsWith("var(") ? getComputedStyle(document.documentElement).getPropertyValue(color.slice(4, -1)).trim() : color;
  const step = w / (HMAX - 1);
  const off = (HMAX - data.length) * step;
  const y = (v) => h - 3 - (Math.min(max, v) / max) * (h - 8);
  g.beginPath();
  data.forEach((v, i) => (i ? g.lineTo(off + i * step, y(v)) : g.moveTo(off, y(v))));
  g.lineWidth = 2; g.strokeStyle = col; g.lineJoin = "round";
  g.shadowColor = col; g.shadowBlur = 8;
  g.stroke();
  g.shadowBlur = 0;
  g.lineTo(off + (data.length - 1) * step, h); g.lineTo(off, h); g.closePath();
  const grad = g.createLinearGradient(0, 0, 0, h);
  grad.addColorStop(0, col + "55"); grad.addColorStop(1, col + "00");
  g.fillStyle = grad; g.fill();
}

const mmss = (ms) => {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

function renderMedia() {
  const m = S.media;
  const has = !!m.title;
  for (const b of document.querySelectorAll(".mini-media, .mediap")) {
    b.querySelector(".mtitle").textContent = has ? m.title : "Çalan bir şey yok";
    b.querySelector(".martist").textContent = has ? (m.artist || m.album || "") : "Bilgisayarda bir şey çalınca burada görünür";
    const src = b.querySelector(".src");
    if (src) src.textContent = m.source ? m.source.replace(/\.exe.*$/i, "").replace(/^.*[\\!]/, "") : "";
    const cv = b.querySelector(".cover");
    const want = m.cover || "";
    if (cv.dataset.src !== want) {
      cv.dataset.src = want;
      cv.innerHTML = want ? `<img src="${want}" alt="">` : "<span>♪</span>";
    }
    b.querySelector('[data-a="toggle"]').innerHTML = m.playing ? ICON.pause : ICON.play;
  }
  tickProgress();
}

function tickProgress() {
  const m = S.media;
  const pos = m.playing ? m.pos + (Date.now() - m.at) : m.pos;
  const dur = m.dur || 0;
  for (const b of document.querySelectorAll(".mini-media, .mediap")) {
    b.querySelector(".cur").textContent = mmss(Math.min(pos, dur || pos));
    b.querySelector(".dur").textContent = mmss(dur);
    b.querySelector(".bar i").style.width = dur ? `${Math.min(100, (pos / dur) * 100)}%` : "0";
  }
}

function renderVol(except) {
  for (const r of document.querySelectorAll(".volr")) {
    if (r !== except && document.activeElement !== r) r.value = Math.round(S.vol.v);
    r.style.setProperty("--p", `${r.value}%`);
  }
  for (const b of document.querySelectorAll(".mutebtn")) b.innerHTML = S.vol.m ? ICON.mute : ICON.vol;
}

// ---------------------------------------------------------------------------
// Hava durumu (telefon doğrudan Open-Meteo'dan alır)
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
    const url = `https://api.open-meteo.com/v1/forecast?latitude=${w.lat}&longitude=${w.lon}&current=temperature_2m,apparent_temperature,weather_code,is_day&daily=temperature_2m_max,temperature_2m_min&forecast_days=1&timezone=auto`;
    const r = await (await fetch(url)).json();
    S.wx = { city: w.city, ...r.current, max: r.daily?.temperature_2m_max?.[0], min: r.daily?.temperature_2m_min?.[0] };
    renderWeather();
  } catch {}
}
function renderWeather() {
  const w = S.wx;
  for (const box of document.querySelectorAll(".wx")) {
    if (!w) { box.innerHTML = ""; continue; }
    const [desc, day, night] = WMO[w.weather_code] || ["", "🌡️"];
    const ic = w.is_day === 0 && night ? night : day;
    box.innerHTML = `<span class="ic">${ic}</span><span class="tmp">${Math.round(w.temperature_2m)}°</span>
      <span class="desc"><b>${esc(desc)}${w.city ? " · " + esc(w.city) : ""}</b>↑${Math.round(w.max)}° ↓${Math.round(w.min)}° · Hissedilen ${Math.round(w.apparent_temperature)}°</span>`;
  }
}

// ---------------------------------------------------------------------------
// Başlangıç
// ---------------------------------------------------------------------------
window.addEventListener("resize", () => renderStats());
showConn(true);
if (location.protocol.startsWith("http")) connect();
else window.__mock?.();
