// Cam Dock arayüzü. Bundler yok; Tauri global API'si (withGlobalTauri) kullanılıyor.
"use strict";

// ---------------------------------------------------------------------------
// Tauri köprüsü (tarayıcıda açılırsa örnek verili sahte köprü)
// ---------------------------------------------------------------------------
const TAURI = window.__TAURI__;
const invoke = TAURI ? TAURI.core.invoke : mockInvoke;
const listen = TAURI ? TAURI.event.listen : async () => () => {};

const $ = (tag, cls, html) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (html != null) e.innerHTML = html;
  return e;
};

const ICON = {
  start: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3.5" y="3.5" width="7" height="7" rx="1.6"/><rect x="13.5" y="3.5" width="7" height="7" rx="1.6"/><rect x="3.5" y="13.5" width="7" height="7" rx="1.6"/><rect x="13.5" y="13.5" width="7" height="7" rx="1.6"/></svg>',
  mail: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="3" y="5" width="18" height="14" rx="2.5"/><path d="M3.5 7l8.5 6 8.5-6"/></svg>',
  spark: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6L5.6 18.4"/></svg>',
  clock: '<svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="8.5"/><path d="M12 7.5V12l3 2"/></svg>',
  swap: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M8 4v16M8 4L5 7M8 4l3 3M16 20V4M16 20l-3-3M16 20l3-3"/></svg>',
  wave: '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M5 10v4M9 7v10M13 9v6M17 6v12M21 10v4"/></svg>',
  prev: '<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M19 5v14l-10-7zM5 5h2v14H5z"/></svg>',
  next: '<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M5 5v14l10-7zM17 5h2v14h-2z"/></svg>',
  play: '<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M7 4l13 8-13 8z"/></svg>',
  pause: '<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M6 4h4v16H6zM14 4h4v16h-4z"/></svg>',
};

let cfg = null;
let info = { monitor: "", primary: true };
const dock = document.getElementById("dock");
const timers = [];
const every = (ms, fn) => { fn(); timers.push(setInterval(fn, ms)); };
// Dock gizliyken (otomatik gizle / tam ekran) sık sorguları durdur.
let dockVisible = true;
// Önceki sorgu bitmeden yenisini başlatma (yavaş bir oynatıcı ya da sistem
// çağrısı sorguları üst üste yığmasın).
const single = (fn) => {
  let busy = false;
  return async () => {
    if (busy) return;
    busy = true;
    try { await fn(); } finally { busy = false; }
  };
};
const whenVisible = (fn) => { const g = single(fn); return () => { if (dockVisible) g(); }; };

// ---------------------------------------------------------------------------
// Tema
// ---------------------------------------------------------------------------
// Geçerli tema Rust'tan gelir: kullanıcı seçimi, Windows teması ya da dinamik
// moddaki saat planı. Tüm pencereler aynı değeri kullanır.
let effDark = matchMedia("(prefers-color-scheme: dark)").matches;
const isDark = () => effDark;
invoke("theme_dark").then((d) => { effDark = !!d; if (cfg) applyTheme(); }).catch(() => {});
listen("theme-dark", (e) => {
  if (!!e.payload === effDark) return;
  effDark = !!e.payload;
  if (cfg) applyTheme(true);
});

async function applyTheme(animate) {
  const root = document.documentElement;
  const dark = isDark();
  if (animate) Dyn.animate(root);
  root.dataset.theme = dark ? "dark" : "light";
  root.dataset.backdrop = cfg.backdrop;
  root.style.setProperty("--accent", cfg.accent || "#2F7BFF");
  root.style.setProperty("--radius", `${cfg.radius ?? 26}px`);
  // Dinamik mod: duvar kâğıdı renkleri (kapalıysa değişkenler temizlenir).
  const tint = await Dyn.apply(root, cfg, dark, "dock");
  invoke("set_theme", { dark, tint }).catch(() => {});
}
Dyn.watch(() => cfg, () => applyTheme(true));

// ---------------------------------------------------------------------------
// Yerleşim: dock'un ölçüsü değişince pencereyi yeniden boyutlandır
// ---------------------------------------------------------------------------
// İlk açılışta dock, uygulama listesi ve simgeler gelmeden ölçülüp ortalanınca
// sonradan genişleyip sağa taşık kalıyordu. İlk yerleşim içerik hazır olunca
// yapılır; sonrakiler kısa bir gecikmeyle toplanır, aynı ölçü tekrar gönderilmez.
let layoutReady = false;
let layoutTimer = null;
let lastSize = "";
let firstAppsDone;
const appsLoaded = new Promise((r) => (firstAppsDone = r));
const pendingIcons = [];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function sendLayout() {
  const r = dock.getBoundingClientRect();
  const size = `${Math.ceil(r.width)}x${Math.ceil(r.height)}`;
  if (size === lastSize) return;
  lastSize = size;
  invoke("layout_dock", { width: Math.ceil(r.width), height: Math.ceil(r.height) }).catch(console.error);
}
function queueLayout() {
  if (!layoutReady) return;
  clearTimeout(layoutTimer);
  layoutTimer = setTimeout(sendLayout, 60);
}
async function firstLayout() {
  await Promise.race([appsLoaded, sleep(2500)]);
  await Promise.race([document.fonts?.ready, sleep(800)]);
  await Promise.race([Promise.allSettled(pendingIcons), sleep(1200)]);
  await sleep(120);
  layoutReady = true;
  sendLayout();
}
new ResizeObserver(queueLayout).observe(dock);

// ---------------------------------------------------------------------------
// Uygulamalar (sabitlenenler + açık olanlar)
// ---------------------------------------------------------------------------
const iconCache = new Map();
function iconFor(item) {
  const key = (item.aumid ? "aumid:" + item.aumid : item.path).toLowerCase();
  if (!iconCache.has(key)) {
    const hwnd = item.wins?.[0]?.hwnd ?? null;
    const p = invoke("app_icon", { path: item.path, aumid: item.aumid || null, hwnd }).catch(() => null);
    iconCache.set(key, p);
    // Bulunamadıysa bir sonraki yenilemede (ör. pencere açılınca) tekrar dene.
    p.then((url) => { if (!url) setTimeout(() => iconCache.delete(key), 5000); });
    if (!layoutReady) pendingIcons.push(p);
  }
  return iconCache.get(key);
}

const isHost = (exe) => /applicationframehost\.exe$/i.test(exe);
// Pencereleri grupla: Store uygulamaları (ve küçültülünce çerçevesinden
// ayrılanlar) kimlikleriyle, diğerleri exe yoluyla.
const groupKey = (w) => (w.aumid ? "aumid:" + w.aumid : w.exe).toLowerCase();

const baseName = (p) => p.split(/[\\/]/).pop().replace(/\.exe$/i, "");

// Uygulama sırası ve önizleme zamanlayıcısı (widget yeniden çizilse de korunur)
const seenOrder = new Map();
const seenWin = new Map();
let seenCounter = 0;
let hoverTimer = null;

function appsWidget() {
  const wrap = $("div", "apps");
  const start = $("button", "app start", `<span class="glyph">${ICON.start}</span>`);
  start.title = "Başlat";
  start.setAttribute("aria-label", "Başlat");
  start.onclick = () => invoke("open_start");
  wrap.append(start);

  const list = $("div", "apps");
  list.style.padding = "0";
  wrap.append(list);

  let lastSig = "";
  let items = [];

  async function refresh() {
    let data;
    try { data = await invoke("list_windows"); } catch { firstAppsDone(); return; }
    const groups = new Map();
    const own = cfg.display?.apps_per_monitor
      ? data.windows.filter((w) => !info.monitor || w.monitor === info.monitor)
      : data.windows;
    for (const w of own) {
      const k = groupKey(w);
      if (!groups.has(k)) groups.set(k, []);
      groups.get(k).push(w);
    }
    // Sabitlenen bir exe'nin pencereleri kimlikli olsa da o sabitlemeye bağlansın.
    const byExe = new Map();
    for (const [k, wins] of groups) {
      const exe = wins[0].exe.toLowerCase();
      // Aynı exe'nin birden çok kimliği olabilir (ör. Chrome ve Chrome uygulamaları):
      // sabitlemeyi kimliksiz gruba ya da ilk bulunana bağla.
      if (!isHost(exe) && (!byExe.has(exe) || !k.startsWith("aumid:"))) byExe.set(exe, k);
    }
    const next = [];
    const used = new Set();
    for (const p of cfg.pinned_resolved.filter((p) => p.exists)) {
      const pk = p.path.toLowerCase();
      const aum = pk.startsWith("shell:appsfolder\\") ? "aumid:" + pk.slice(17) : null;
      const k = (aum && groups.has(aum)) ? aum : groups.has(pk) ? pk : byExe.get(pk) || pk;
      used.add(k);
      next.push({ key: k, name: p.name, path: p.path, args: p.args, aumid: aum ? aum.slice(6) : "", wins: groups.get(k) || [], pinned: true });
    }
    if (cfg.show_running) {
      const running = [];
      for (const [k, wins] of groups) {
        if (used.has(k)) continue;
        const w0 = wins[0];
        const name = w0.aumid || isHost(w0.exe) ? (w0.title.split(" - ").pop() || w0.title) : baseName(w0.exe);
        if (!seenOrder.has(k)) seenOrder.set(k, ++seenCounter);
        running.push({ key: k, name, path: w0.exe, args: null, aumid: w0.aumid, wins });
      }
      // Açık uygulamalar ilk göründükleri sırada kalır (öne gelen öne kaymaz).
      running.sort((a, b) => seenOrder.get(a.key) - seenOrder.get(b.key));
      next.push(...running);
    }
    // Kapanan uygulamanın sırası unutulsun; tekrar açılınca sona eklenir.
    for (const k of [...seenOrder.keys()]) if (!groups.has(k)) seenOrder.delete(k);
    // Pencereler: Z sırası (tıklama için) ve ilk görülme sırası (önizleme için)
    for (const it of next) {
      for (const w of it.wins) if (!seenWin.has(w.hwnd)) seenWin.set(w.hwnd, ++seenCounter);
      it.stable = [...it.wins].sort((a, b) => seenWin.get(a.hwnd) - seenWin.get(b.hwnd));
    }
    items = next;
    const activeKey = items.find((i) => i.wins.some((w) => w.hwnd === data.active))?.key || "";
    const sig = items.map((i) => `${i.key}:${i.wins.length}`).join("|") + "#" + activeKey;
    if (sig === lastSig) return firstAppsDone();
    lastSig = sig;
    render(data.active);
    firstAppsDone();
  }

  function render(active) {
    list.replaceChildren();
    for (const item of items) {
      const b = $("button", "app");
      const isActive = item.wins.some((w) => w.hwnd === active);
      if (isActive) b.classList.add("is-active");
      b.setAttribute("aria-label", item.name);
      b.title = item.wins.length
        ? `${item.name}\n${item.wins.map((w) => "• " + w.title).join("\n")}`
        : item.name;
      const glyph = $("span", "glyph", item.name.slice(0, 1).toUpperCase());
      b.append(glyph);
      iconFor(item).then((url) => {
        if (!url) return;
        const img = new Image();
        img.src = url;
        img.alt = "";
        glyph.replaceWith(img);
      });
      if (item.wins.length) b.append($("span", "run"));
      b.style.setProperty("--i", list.childElementCount);

      b.onclick = () => {
        clearTimeout(hoverTimer);
        const sp = $("span", "splash");
        b.append(sp);
        setTimeout(() => sp.remove(), 650);
        if (!item.wins.length) return invoke("launch_app", { path: item.path, args: item.args });
        // Öndeyse küçült, küçültülmüşse eski konumuna getir, arkadaysa öne getir.
        invoke("activate_app", { hwnds: item.wins.map((w) => w.hwnd) }).then(() => setTimeout(refresh, 300));
      };
      // Birden çok penceresi olan uygulamanın üstüne gelince tüm pencereleri göster.
      b.onmouseenter = () => {
        clearTimeout(hoverTimer);
        if (item.wins.length < 2) return;
        invoke("popover_dock_hover", { hover: true }).catch(() => {});
        hoverTimer = setTimeout(async () => {
          const r = b.getBoundingClientRect();
          const icon = await iconFor(item);
          const cur = items.find((i) => i.key === item.key) || item;
          invoke("popover_open", {
            kind: "windows",
            key: item.key,
            anchor: r.left + r.width / 2,
            payload: {
              name: item.name,
              icon,
              windows: cur.stable.map((w) => ({ hwnd: w.hwnd, title: w.title, minimized: w.minimized })),
            },
          }).catch(console.error);
        }, 380);
      };
      b.onmouseleave = () => {
        clearTimeout(hoverTimer);
        if (item.wins.length >= 2) invoke("popover_dock_hover", { hover: false }).catch(() => {});
      };
      // Sağ tık: uygulamaya özel menü (yeni pencere, gizli pencere, sabitle…)
      b.oncontextmenu = (e) => {
        e.preventDefault();
        e.stopPropagation();
        clearTimeout(hoverTimer);
        const cur = items.find((i) => i.key === item.key) || item;
        invoke("app_menu", {
          item: {
            name: item.name,
            path: item.path,
            args: item.args || null,
            aumid: item.aumid || "",
            windows: (cur.stable || cur.wins).map((w) => [w.hwnd, w.title]),
            pinned: !!item.pinned,
          },
        }).catch(console.error);
      };
      // Orta tık: yeni pencere aç
      b.onauxclick = (e) => {
        if (e.button === 1) invoke("launch_app", { path: item.path, args: item.args });
      };
      list.append(b);
    }
  }

  every(1000, whenVisible(refresh));
  return wrap;
}

// ---------------------------------------------------------------------------
// Widget'lar
// ---------------------------------------------------------------------------
const trDate = (d, o) => d.toLocaleDateString("tr-TR", o);

function calendarWidget() {
  const c = $("button", "cell w-calendar");
  c.setAttribute("aria-label", "Takvimi aç");
  c.innerHTML = `
    <span class="datebox"><span class="dow"></span><span class="day"></span></span>
    <span class="stack"><span class="t-strong"></span><span class="t-sub"></span></span>`;
  const [dow, day, title, sub] = ["dow", "day", "t-strong", "t-sub"].map((k) => c.querySelector("." + k));
  every(60_000, () => {
    const d = new Date();
    dow.textContent = trDate(d, { weekday: "short" }).toLocaleUpperCase("tr-TR").slice(0, 3);
    day.textContent = d.getDate();
    title.textContent = trDate(d, { day: "numeric", month: "long", weekday: "long" });
    const h = CAL.holiday(d);
    const next = CAL.upcoming(d, 1)[0];
    sub.textContent = h
      ? h.name
      : next && next.days <= 45
        ? `${CAL.isoWeek(d)}. hafta · ${next.name.split(" ")[0]} ${next.days} gün sonra`
        : `${CAL.isoWeek(d)}. hafta · yılın ${CAL.dayOfYear(d)}. günü`;
  });
  c.onclick = async () => {
    const r = c.getBoundingClientRect();
    const res = await invoke("popover_open", { kind: "calendar", key: "cal", anchor: r.left + r.width / 2, payload: null }).catch(() => "");
    c.classList.toggle("active", res === "opened" || res === "open");
  };
  calendarCell = c;
  return c;
}
let calendarCell = null;

const LOCK_ICON = {
  on: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="11" width="14" height="10" rx="2.5"/><path d="M8 11V7.5a4 4 0 0 1 8 0V11"/></svg>',
  off: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="11" width="14" height="10" rx="2.5"/><path d="M8 11V7.5a4 4 0 0 1 7.5-1.9"/></svg>',
};

// Kilit: kilitliyken dock hep görünür ve pencereler altına girmez;
// açıkken dock otomatik gizlenir, pencereler tüm ekranı kaplar.
function lockWidget() {
  const b = $("button", "lock" + (cfg.dock_locked ? " on" : ""), cfg.dock_locked ? LOCK_ICON.on : LOCK_ICON.off);
  const label = cfg.dock_locked
    ? "Dock kilitli: hep görünür, pencereler altına girmez. Tıkla: otomatik gizle"
    : "Otomatik gizleme açık: pencereler tüm ekranı kaplar. Tıkla: kilitle";
  b.title = label;
  b.setAttribute("aria-label", label);
  b.setAttribute("aria-pressed", cfg.dock_locked ? "true" : "false");
  b.onclick = () => invoke("toggle_lock");
  return b;
}

function mailWidget() {
  const c = $("div", "cell");
  c.title = "Önizleme — örnek veri. Outlook/Gmail bağlantısı Faz 2'de.";
  c.innerHTML = `
    <span class="tile" style="background:#2F7BFF">${ICON.mail}</span>
    <span class="stack"><span class="t-big">4</span><span class="t-sub">Okunmamış · örnek</span></span>`;
  return c;
}

function claudeWidget() {
  const c = $("div", "cell");
  c.title = "Önizleme — örnek veri.";
  const row = (l, p, v) =>
    `<span class="lbl">${l}</span><span class="track"><span style="width:${p}%"></span></span><span class="val">${v}</span>`;
  c.innerHTML = `
    <span class="tile" style="background:#D97757">${ICON.spark}</span>
    <span class="usage">${row("Oturum", 34, "%34")}${row("Haftalık", 58, "%58")}${row("Opus", 22, "%22")}</span>`;
  return c;
}

// Ekran süresi + sistem kaynakları: hücre ekran süresi → CPU → RAM → GPU →
// Disk arasında döner (fare üstündeyken durur, tekerlekle elle geçilir);
// tıklayınca üstte grafikli kullanım paneli açılır.
const METRICS = [
  { k: "cpu", name: "İşlemci", short: "CPU" },
  { k: "ram", name: "Bellek", short: "RAM" },
  { k: "gpu", name: "Ekran kartı", short: "GPU" },
  { k: "disk", name: "Disk etkinliği", short: "Disk" },
];
const fmtDur = (secs) => {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h ? `${h}s ${m}dk` : `${m}dk`;
};
const ring = (pct, cls) => {
  const r = 15, c = 2 * Math.PI * r;
  const v = Math.max(0, Math.min(100, pct));
  return `<svg class="ring ${cls}" viewBox="0 0 40 40" width="40" height="40" aria-hidden="true">
    <circle cx="20" cy="20" r="${r}" class="ring-track"/>
    <circle cx="20" cy="20" r="${r}" class="ring-val" stroke-dasharray="${c}" stroke-dashoffset="${c * (1 - v / 100)}"/>
  </svg><b class="ring-num">${Math.round(v)}<small>%</small></b>`;
};
function sparkPath(vals, w, h) {
  if (vals.length < 2) return { line: "", area: "" };
  const step = w / (vals.length - 1);
  const pts = vals.map((v, i) => [i * step, h - 1 - (Math.max(0, Math.min(100, v)) / 100) * (h - 2)]);
  const line = pts.map((p, i) => `${i ? "L" : "M"}${p[0].toFixed(1)},${p[1].toFixed(1)}`).join("");
  return { line, area: `${line}L${w},${h}L0,${h}Z` };
}

let screenCell = null;
function screenTimeWidget() {
  const c = $("button", "cell w-screen");
  c.setAttribute("aria-label", "Ekran süresi ve sistem kaynakları — kullanım panelini aç");
  c.innerHTML = `<div class="sv-view"></div><div class="sv-dots" aria-hidden="true"></div>`;
  const view = c.querySelector(".sv-view");
  const dots = c.querySelector(".sv-dots");
  const hist = { cpu: [], ram: [], gpu: [], disk: [] };
  let st = null;
  let stats = null;
  let idx = 0;
  let paused = false;
  let lastSwitch = Date.now();

  const pages = () => ["screen", ...METRICS.filter((m) => !stats || stats[m.k] >= 0).map((m) => m.k)];

  function build(page) {
    const el = $("div", "sv-item");
    el.dataset.page = page;
    if (page === "screen") {
      el.innerHTML = `
        <span class="sv-icon">${ICON.clock}</span>
        <span class="stack"><span class="t-sub">Ekran süresi</span><span class="t-big">–</span></span>
        <span class="bars" aria-hidden="true"></span>`;
    } else {
      const m = METRICS.find((x) => x.k === page);
      el.innerHTML = `
        <span class="ring-wrap m-${m.k}"></span>
        <span class="stack"><span class="t-sub">${m.name}</span><span class="t-strong sv-detail"></span></span>
        <svg class="spark m-${m.k}" viewBox="0 0 58 30" width="58" height="30" preserveAspectRatio="none" aria-hidden="true"><path class="sp-area"/><path class="sp-line"/></svg>`;
    }
    fill(el);
    return el;
  }

  function fill(el) {
    const page = el.dataset.page;
    if (page === "screen") {
      if (!st) return;
      el.querySelector(".t-big").textContent = fmtDur(st.seconds);
      const bars = el.querySelector(".bars");
      const now = new Date().getHours();
      bars.replaceChildren();
      for (let i = 6; i >= 0; i--) {
        const hour = now - i;
        const secs = hour >= 0 ? st.hours[hour] || 0 : 0;
        const s = $("span", i === 0 ? "" : "dim");
        s.style.height = `${Math.max(10, Math.min(100, (secs / 3600) * 100))}%`;
        bars.append(s);
      }
      return;
    }
    if (!stats) return;
    const v = stats[page];
    el.querySelector(".ring-wrap").innerHTML = ring(v, "m-" + page);
    el.querySelector(".sv-detail").textContent =
      page === "ram" ? `${stats.ram_used_gb.toFixed(1)} / ${stats.ram_total_gb.toFixed(0)} GB`
      : page === "cpu" ? (v >= 75 ? "Yoğun" : v >= 40 ? "Orta" : "Rahat")
      : page === "gpu" ? (v >= 60 ? "Yoğun" : v >= 15 ? "Çalışıyor" : "Boşta")
      : (v >= 60 ? "Meşgul" : v >= 10 ? "Okuma/yazma" : "Sakin");
    const { line, area } = sparkPath(hist[page], 58, 30);
    el.querySelector(".sp-line").setAttribute("d", line);
    el.querySelector(".sp-area").setAttribute("d", area);
  }

  function drawDots() {
    const p = pages();
    dots.replaceChildren(...p.map((_, i) => $("i", i === idx ? "on" : "")));
  }

  function go(next, dir = 1) {
    const p = pages();
    idx = ((next % p.length) + p.length) % p.length;
    lastSwitch = Date.now();
    const el = build(p[idx]);
    const prev = view.lastElementChild;
    if (prev) {
      el.classList.add(dir > 0 ? "enter" : "enter-up");
      view.append(el);
      void el.offsetWidth;
      el.classList.remove("enter", "enter-up");
      prev.classList.add(dir > 0 ? "leave" : "leave-down");
      setTimeout(() => prev.remove(), 450);
    } else view.append(el);
    drawDots();
  }

  c.onmouseenter = () => (paused = true);
  c.onmouseleave = () => (paused = false);
  c.onwheel = (e) => {
    e.preventDefault();
    if (Date.now() - lastSwitch < 250) return;
    go(idx + (e.deltaY > 0 ? 1 : -1), e.deltaY > 0 ? 1 : -1);
  };
  c.onclick = async () => {
    const r = c.getBoundingClientRect();
    const res = await invoke("popover_open", { kind: "usage", key: "usage", anchor: r.left + r.width / 2, payload: null }).catch(() => "");
    c.classList.toggle("active", res === "opened" || res === "open");
  };

  every(30_000, async () => {
    try { st = await invoke("screen_time"); } catch { return; }
    const cur = view.lastElementChild;
    if (cur?.dataset.page === "screen") fill(cur);
  });
  every(1500, whenVisible(async () => {
    try { stats = await invoke("sys_stats"); } catch { return; }
    for (const m of METRICS) {
      hist[m.k].push(Math.max(0, stats[m.k]));
      if (hist[m.k].length > 30) hist[m.k].shift();
    }
    const cur = view.lastElementChild;
    if (cur && cur.dataset.page !== "screen") fill(cur);
    c.title = `Ekran süresi ${st ? fmtDur(st.seconds) : "–"} · CPU %${Math.round(stats.cpu)} · RAM ${stats.ram_used_gb.toFixed(1)}/${stats.ram_total_gb.toFixed(0)} GB` +
      (stats.gpu >= 0 ? ` · GPU %${Math.round(stats.gpu)}` : "") + (stats.disk >= 0 ? ` · Disk %${Math.round(stats.disk)}` : "") + "\nAyrıntılar için tıkla";
  }));
  // Ekran süresi biraz daha uzun kalır (8 sn), kaynaklar 4'er sn.
  timers.push(setInterval(() => {
    if (paused || !dockVisible) return;
    const limit = pages()[idx] === "screen" ? 8000 : 4000;
    if (Date.now() - lastSwitch >= limit) go(idx + 1);
  }, 500));
  go(0);
  screenCell = c;
  return c;
}

// ---------------------------------------------------------------------------
// Piyasa ve haberler: dolar, euro, gram altın ve haber başlıkları dönüşümlü
// ---------------------------------------------------------------------------
let tickerCell = null;
function tickerWidget() {
  const t = cfg.ticker || { markets: ["USD", "EUR", "gram-altin"], news: true, feeds: [], interval_sec: 6, news_per_round: 3 };
  const c = $("button", "cell ticker");
  const view = $("div", "tk-view");
  c.append(view);
  let markets = [];
  let news = [];
  let newsCursor = 0;
  let queue = [];
  let paused = false;
  let current = null;

  async function loadMarkets() {
    try {
      const data = JSON.parse(await invoke("fetch_text", { url: "https://finans.truncgil.com/today.json" }));
      markets = t.markets
        .filter((k) => data[k] && MARKETS[k])
        .map((k) => ({ kind: "market", key: k, value: trNum(data[k]["Satış"]), change: trNum(data[k]["Değişim"]) }));
    } catch (e) {
      console.warn("piyasa verisi alınamadı", e);
    }
  }

  async function loadNews() {
    if (!t.news || !t.feeds.length) { news = []; return; }
    const all = [];
    for (const url of t.feeds) {
      try {
        all.push(...parseFeed(await invoke("fetch_text", { url }), new URL(url).hostname));
      } catch (e) {
        console.warn("haber kaynağı okunamadı", url, e);
      }
    }
    const seen = new Set();
    news = all
      .filter((n) => !seen.has(n.title) && seen.add(n.title))
      .sort((a, b) => (b.date || 0) - (a.date || 0))
      .slice(0, 40);
    newsCursor = 0;
  }

  function nextRound() {
    const round = [...markets];
    const n = Math.min(t.news_per_round || 3, news.length);
    for (let i = 0; i < n; i++) round.push(news[(newsCursor + i) % news.length]);
    newsCursor = news.length ? (newsCursor + n) % news.length : 0;
    return round;
  }

  function render(item) {
    const el = $("div", "tk-item");
    if (!item) {
      el.innerHTML = `<span class="stack"><span class="t-sub">Piyasa ve haberler</span><span class="t-strong">Veriler yükleniyor…</span></span>`;
      return el;
    }
    if (item.kind === "market") {
      const m = MARKETS[item.key];
      const dir = item.change > 0 ? "up" : item.change < 0 ? "down" : "flat";
      const arrow = dir === "up" ? "▲" : dir === "down" ? "▼" : "•";
      el.innerHTML = `
        <span class="tile tk-glyph" style="background:${m.bg}">${m.glyph}</span>
        <span class="stack"><span class="t-sub">${m.name} · ${m.pair}</span><span class="t-big">${fmt2(item.value)} ₺</span></span>
        <span class="chg ${dir}">${arrow} %${fmt2(Math.abs(item.change))}</span>`;
      el.title = `${m.name}: ${fmt2(item.value)} ₺ (günlük %${fmt2(item.change)}) — kaynak: Truncgil Finans`;
    } else {
      el.classList.add("news");
      const ago = item.date && !isNaN(item.date) ? ` · ${timeAgo(item.date)}` : "";
      el.innerHTML = `
        <span class="tile tk-glyph tk-news">${NEWS_ICON}</span>
        <span class="stack"><span class="t-sub"></span><span class="t-news"></span></span>`;
      el.querySelector(".t-sub").textContent = item.source + ago;
      el.querySelector(".t-news").textContent = item.title;
      el.title = `${item.title}\n${item.source}${ago}\nTüm haberler için tıkla`;
    }
    return el;
  }

  function show(item) {
    current = item;
    const next = render(item);
    const prev = view.lastElementChild;
    if (!prev) { view.append(next); return; }
    next.classList.add("enter");
    view.append(next);
    void next.offsetWidth; // animasyonu başlat
    next.classList.remove("enter");
    prev.classList.add("leave");
    setTimeout(() => prev.remove(), 500);
  }

  function advance() {
    if (!queue.length) queue = nextRound();
    show(queue.shift() || null);
  }

  c.onmouseenter = () => (paused = true);
  c.onmouseleave = () => (paused = false);
  // Tıklayınca tarayıcıya gitmek yerine üstte haber ve piyasa paneli açılır.
  c.onclick = async () => {
    const r = c.getBoundingClientRect();
    const res = await invoke("popover_open", {
      kind: "news", key: "news", anchor: r.left + r.width / 2,
      payload: { markets: t.markets, feeds: t.feeds, focus: current?.kind === "news" ? current.link : null },
    }).catch(() => "");
    c.classList.toggle("active", res === "opened" || res === "open");
  };
  c.onwheel = (e) => { e.preventDefault(); advance(); };

  show(null);
  (async () => {
    await Promise.all([loadMarkets(), loadNews()]);
    queue = [];
    advance();
  })();
  timers.push(setInterval(() => !paused && advance(), Math.max(3, t.interval_sec || 6) * 1000));
  timers.push(setInterval(loadMarkets, 5 * 60_000));
  timers.push(setInterval(loadNews, 15 * 60_000));
  tickerCell = c;
  return c;
}

// ---------------------------------------------------------------------------
// Medya: birden çok oynatıcı varsa tekerlekle ya da noktalarla dikey kaydır
// ---------------------------------------------------------------------------
function mediaWidget() {
  const c = $("div", "cell media idle");
  c.innerHTML = `<div class="m-view"></div><div class="m-dots" aria-hidden="true"></div>`;
  const view = c.querySelector(".m-view");
  const dots = c.querySelector(".m-dots");
  let sessions = [];
  let selected = null; // oturum kimliği
  let pickedAt = 0; // kullanıcı en son ne zaman elle seçti
  let shownId = null;
  let thumbs = new Map();

  function item(m) {
    const el = $("div", "m-item");
    el.innerHTML = `
      <span class="art" aria-hidden="true">${ICON.wave}</span>
      <span class="stack meta"><span class="t-strong"></span><span class="t-sub"></span></span>
      <button class="ctrl" data-a="prev" aria-label="Önceki">${ICON.prev}</button>
      <button class="ctrl main" data-a="toggle" aria-label="Oynat/Duraklat">${m && m.playing ? ICON.pause : ICON.play}</button>
      <button class="ctrl" data-a="next" aria-label="Sonraki">${ICON.next}</button>
      <span class="progress" aria-hidden="true"><span></span></span>`;
    el.querySelectorAll(".ctrl").forEach((b) => {
      b.onclick = async (e) => {
        e.stopPropagation();
        if (!selected) return;
        await invoke("media_control_id", { id: selected, action: b.dataset.a });
        setTimeout(poll, 250);
      };
    });
    return el;
  }

  function fill(el, m) {
    const [title, artist] = [el.querySelector(".t-strong"), el.querySelector(".t-sub")];
    if (!m) {
      title.textContent = "Çalan bir şey yok";
      artist.textContent = "";
      return;
    }
    title.textContent = m.title || "Bilinmeyen parça";
    artist.textContent = m.artist || m.source.replace(/\.exe$/i, "").split("!").pop();
    el.title = [m.title, m.artist, m.album].filter(Boolean).join(" — ");
    el.querySelector(".ctrl.main").innerHTML = m.playing ? ICON.pause : ICON.play;
    el.querySelector(".progress > span").style.width = m.duration_ms > 0 ? `${(m.position_ms / m.duration_ms) * 100}%` : "0%";
    const art = el.querySelector(".art");
    const k = `${m.id}\u0001${m.track_key}`;
    const apply = (url) => {
      art.style.backgroundImage = url ? `url("${url}")` : "";
      art.classList.toggle("has-img", !!url);
    };
    if (thumbs.has(k)) apply(thumbs.get(k));
    else {
      thumbs.set(k, null);
      invoke("media_thumbnail_id", { id: m.id, trackKey: m.track_key })
        .then((url) => { thumbs.set(k, url); if (shownId === m.id) apply(url); })
        .catch(() => {});
    }
  }

  function show(m, dir) {
    const prev = view.lastElementChild;
    const same = prev && shownId === (m ? m.id : null);
    if (same) return fill(prev, m);
    const el = item(m);
    fill(el, m);
    shownId = m ? m.id : null;
    if (!prev || !dir) {
      view.replaceChildren(el);
      return;
    }
    el.classList.add(dir > 0 ? "from-below" : "from-above");
    view.append(el);
    void el.offsetWidth;
    el.classList.remove("from-below", "from-above");
    prev.classList.add(dir > 0 ? "to-above" : "to-below");
    setTimeout(() => prev.remove(), 380);
  }

  function renderDots() {
    dots.replaceChildren();
    if (sessions.length < 2) return;
    for (const m of sessions) {
      const d = $("span", m.id === selected ? "on" : "");
      if (m.playing) d.classList.add("playing");
      dots.append(d);
    }
  }

  function step(dir) {
    if (sessions.length < 2) return;
    const i = Math.max(0, sessions.findIndex((m) => m.id === selected));
    const next = sessions[(i + dir + sessions.length) % sessions.length];
    selected = next.id;
    pickedAt = Date.now();
    show(next, dir);
    renderDots();
  }

  async function poll() {
    let list;
    try { list = await invoke("media_list"); } catch { return; }
    sessions = list.filter((m) => m.active);
    c.classList.toggle("idle", !sessions.length);
    c.classList.toggle("multi", sessions.length > 1);
    if (!sessions.length) {
      selected = null;
      show(null);
      renderDots();
      return;
    }
    const keep = sessions.find((m) => m.id === selected);
    const userRecent = Date.now() - pickedAt < 15000;
    let pick = keep;
    if (!userRecent || !keep) {
      // Otomatik: çalanı (Windows'un "şu anki"sini önceleyerek) göster
      pick = sessions.find((m) => m.playing && m.current) || sessions.find((m) => m.playing) || keep || sessions.find((m) => m.current) || sessions[0];
    }
    const oldIdx = sessions.findIndex((m) => m.id === selected);
    const newIdx = sessions.findIndex((m) => m.id === pick.id);
    selected = pick.id;
    show(pick, shownId && shownId !== pick.id ? (newIdx >= oldIdx ? 1 : -1) : 0);
    renderDots();
  }

  c.addEventListener("wheel", (e) => {
    e.preventDefault();
    if (Math.abs(e.deltaY) < 4) return;
    step(e.deltaY > 0 ? 1 : -1);
  }, { passive: false });
  dots.onclick = () => step(1);
  c.title = "Birden çok oynatıcı varsa tekerlekle kaydır";

  show(null);
  every(1000, whenVisible(poll));
  return c;
}

// ---------------------------------------------------------------------------
// Sistem kaynakları: CPU, RAM, GPU, Disk
// ---------------------------------------------------------------------------
// Saat + hava durumu: sıcaklık ve simge; tıklayınca saatlik ve 5 günlük tahmin.
let clockCell = null;
function clockWidget() {
  const withWx = cfg.weather?.enabled !== false;
  const c = $(withWx ? "button" : "div", "cell plain clock" + (withWx ? " has-wx" : ""));
  c.innerHTML = `${withWx ? `<span class="wx"><span class="wx-i"></span><span class="stack"><span class="wx-t">–°</span><span class="t-sub wx-c"></span></span></span>` : ""}
    <span class="stack"><span class="t-big"></span><span class="t-sub"></span></span>`;
  const time = c.querySelector(":scope > .stack .t-big");
  const date = c.querySelector(":scope > .stack .t-sub");
  every(5_000, () => {
    const d = new Date();
    time.textContent = d.toLocaleTimeString("tr-TR", { hour: "2-digit", minute: "2-digit" });
    date.textContent = trDate(d, { day: "numeric", month: "short", year: "numeric" });
  });
  if (withWx) {
    c.setAttribute("aria-label", "Saat ve hava durumu — tahmini aç");
    const icon = c.querySelector(".wx-i"), temp = c.querySelector(".wx-t"), city = c.querySelector(".wx-c");
    const load = async () => {
      try {
        const w = await Weather.get(cfg);
        const cur = w.current;
        icon.innerHTML = wxIcon(cur.weather_code, cur.is_day === 1, 28);
        temp.textContent = `${Math.round(cur.temperature_2m)}°`;
        city.textContent = w.city || wxText(cur.weather_code);
        const today = w.daily;
        c.title = `${w.city || ""} · ${wxText(cur.weather_code)}, ${Math.round(cur.temperature_2m)}° (hissedilen ${Math.round(cur.apparent_temperature)}°)\n` +
          `Bugün en yüksek ${Math.round(today.temperature_2m_max[0])}°, en düşük ${Math.round(today.temperature_2m_min[0])}°\nTahmin için tıkla`;
      } catch (e) {
        console.warn("hava durumu alınamadı", e);
      }
    };
    load();
    timers.push(setInterval(load, 15 * 60_000));
    c.onclick = async () => {
      const r = c.getBoundingClientRect();
      const res = await invoke("popover_open", { kind: "weather", key: "weather", anchor: r.left + r.width / 2, payload: null }).catch(() => "");
      c.classList.toggle("active", res === "opened" || res === "open");
    };
  }
  clockCell = c;
  return c;
}

const SYS_ICON = {
  tray: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 15l6-6 6 6"/></svg>',
  quick: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M2 8.5a15 15 0 0 1 20 0M5 12a10 10 0 0 1 14 0M8.5 15.5a5 5 0 0 1 7 0"/><circle cx="12" cy="19" r="1" fill="currentColor"/></svg>',
  notifications: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/></svg>',
  taskview: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="3" y="5" width="8" height="14" rx="2"/><rect x="13" y="5" width="8" height="6" rx="2"/><rect x="13" y="13" width="8" height="6" rx="2"/></svg>',
};

function systemWidget() {
  const c = $("div", "cell sys");
  const items = [
    ["tray", "Arka planda çalışan uygulamalar (sistem tepsisi)"],
    ["quick", "Hızlı ayarlar: Wi-Fi, ses, parlaklık (Win+A)"],
    ["notifications", "Bildirimler (Win+N)"],
    ["taskview", "Görev görünümü: açık pencereler ve masaüstleri (Win+Tab)"],
  ];
  for (const [action, label] of items) {
    const b = $("button", "", SYS_ICON[action]);
    b.title = label;
    b.setAttribute("aria-label", label);
    b.onclick = () => {
      if (action === "tray") {
        const r = b.getBoundingClientRect();
        invoke("popover_open", { kind: "tray", key: "tray", anchor: r.left + r.width / 2, payload: null });
      } else invoke("system_action", { action });
    };
    c.append(b);
  }
  return c;
}

const WIDGETS = {
  system: systemWidget,
  lock: lockWidget,
  calendar: calendarWidget,
  mail: mailWidget,
  claude: claudeWidget,
  screentime: screenTimeWidget,
  ticker: tickerWidget,
  currency: tickerWidget, // eski ayar dosyaları için
  media: mediaWidget,
  clock: clockWidget,
};

// ---------------------------------------------------------------------------
// Kurulum
// ---------------------------------------------------------------------------
function render() {
  timers.splice(0).forEach(clearInterval);
  dock.replaceChildren(appsWidget(), $("span", "sep"));
  for (const name of cfg.widgets) {
    const make = WIDGETS[name];
    if (make) dock.append(make());
  }
  queueLayout();
}

async function init() {
  info = (await invoke("dock_info").catch(() => null)) || info;
  cfg = await invoke("get_config");
  applyTheme();
  render();
  firstLayout();
}

dock.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  invoke("show_menu").catch(console.error);
});

// Ekran eklenip çıkarılınca ya da ayar değişince yeniden yerleş.
listen("relayout", () => { lastSize = ""; queueLayout(); });
listen("dock-visibility", (e) => { dockVisible = !!e.payload; });
listen("popover-closed", (e) => {
  const cell = { calendar: calendarCell, usage: screenCell, news: tickerCell, weather: clockCell }[e.payload];
  cell?.classList.remove("active");
});

listen("config-changed", async () => {
  const before = JSON.stringify([cfg?.dynamic, cfg?.theme]);
  cfg = await invoke("get_config");
  iconCache.clear();
  effDark = !!(await invoke("theme_dark").catch(() => effDark));
  applyTheme(before !== JSON.stringify([cfg.dynamic, cfg.theme]));
  render();
});

init();

// ---------------------------------------------------------------------------
// Tarayıcıda önizleme için sahte köprü (Tauri dışında açılırsa)
// ---------------------------------------------------------------------------
async function mockInvoke(cmd, args) {
  const params = new URLSearchParams(location.search);
  switch (cmd) {
    case "get_config":
      return {
        theme: params.get("theme") || "system",
        backdrop: "none",
        radius: 26,
        accent: "#2F7BFF",
        show_running: true,
        display: { mode: "primary", selected: [], apps_per_monitor: false },
        widgets: (params.get("widgets") || "calendar,screentime,ticker,media,system,clock,lock").split(","),
        dock_locked: params.get("locked") !== "0",
        dynamic: { enabled: params.has("wall"), tint: Number(params.get("tint") || 45), auto_theme: false, schedule: "sun", light_at: "07:00", dark_at: "19:30" },
        weather: { enabled: true, auto_location: true, city: "İstanbul", lat: 41, lon: 29 },
        ticker: { markets: ["USD", "EUR", "gram-altin"], news: true, feeds: ["https://news.google.com/rss"], interval_sec: Number(params.get("iv") || 6), news_per_round: 3 },
        pinned_resolved: ["Dosya Gezgini", "Chrome", "VS Code", "Spotify", "Telegram"].map((n) => ({
          name: n, path: `C:\\${n}.exe`, args: null, exists: true,
        })),
      };
    case "list_windows":
      return {
        active: 2,
        windows: [
          { hwnd: 1, title: "Belgeler", exe: "C:\\Dosya Gezgini.exe", minimized: false, aumid: "" },
          { hwnd: 4, title: "WhatsApp", exe: "C:\\Windows\\System32\\ApplicationFrameHost.exe", minimized: true, aumid: "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App" },
          { hwnd: 2, title: "Cam Dock — Visual Studio Code", exe: "C:\\VS Code.exe", minimized: false },
          { hwnd: 3, title: "Spotify Premium", exe: "C:\\Spotify.exe", minimized: false },
        ],
      };
    case "screen_time":
      return { seconds: 6 * 3600 + 5 * 60, hours: [0,0,0,0,0,0,0,0,900,2400,3300,3000,1500,3600,2800,3400,2000,2600,0,0,0,0,0,0] };
    case "media_list":
      return [
        { id: "Spotify.exe#0", current: true, active: true, title: "Gece Sürüşü", artist: "Örnek Sanatçı", album: "", playing: true, position_ms: 134000, duration_ms: 218000, source: "Spotify.exe", track_key: "a" },
        { id: "Chrome#0", current: false, active: true, title: "Kurtlar Vadisi Pusu | 52. Bölüm", artist: "YouTube", album: "", playing: false, position_ms: 600000, duration_ms: 2400000, source: "Chrome", track_key: "b" },
      ];
    case "theme_dark":
      return (params.get("theme") || "dark") === "dark";
    case "wallpaper":
      return MOCK.wallpaper(params.get("wall"));
    case "sys_stats":
      return { cpu: 23, ram: 61, ram_used_gb: 9.7, ram_total_gb: 16, gpu: 8, disk: 3 };
    case "media_state":
      return { active: true, title: "Gece Sürüşü", artist: "Örnek Sanatçı", album: "", playing: true, position_ms: 134000, duration_ms: 218000, source: "Spotify.exe", track_key: "x" };
    case "fetch_text":
      if (args.url.includes("open-meteo") || args.url.includes("geojs")) return MOCK.fetch(args.url);
      if (args.url.includes("truncgil"))
        return JSON.stringify({
          USD: { "Satış": "49,8407", "Değişim": "%0,00" },
          EUR: { "Satış": "55,6748", "Değişim": "%0,18" },
          "gram-altin": { "Satış": "6.450,55", "Değişim": "%-1,03" },
        });
      return `<rss><channel><title>Google Haberler</title>
        <item><title>Merkez Bankası faiz kararını açıkladı - Örnek Kaynak</title><link>https://example.com/1</link><source>Örnek Kaynak</source><pubDate>${new Date(Date.now() - 12 * 60000).toUTCString()}</pubDate></item>
        <item><title>İzmir'de hafta sonu hava durumu: sağanak bekleniyor - Örnek Haber</title><link>https://example.com/2</link><source>Örnek Haber</source><pubDate>${new Date(Date.now() - 95 * 60000).toUTCString()}</pubDate></item>
      </channel></rss>`;
    default:
      return null;
  }
}
