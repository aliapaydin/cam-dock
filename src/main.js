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
const whenVisible = (fn) => () => { if (dockVisible) fn(); };

// ---------------------------------------------------------------------------
// Tema
// ---------------------------------------------------------------------------
const darkQuery = matchMedia("(prefers-color-scheme: dark)");
const isDark = () => cfg.theme === "dark" || (cfg.theme !== "light" && darkQuery.matches);

function applyTheme() {
  const root = document.documentElement;
  const dark = isDark();
  root.dataset.theme = dark ? "dark" : "light";
  root.dataset.backdrop = cfg.backdrop;
  root.style.setProperty("--accent", cfg.accent || "#2F7BFF");
  root.style.setProperty("--radius", `${cfg.radius ?? 26}px`);
  invoke("set_theme", { dark }).catch(() => {});
}
darkQuery.addEventListener("change", () => cfg && applyTheme());

// ---------------------------------------------------------------------------
// Yerleşim: dock'un ölçüsü değişince pencereyi yeniden boyutlandır
// ---------------------------------------------------------------------------
let layoutQueued = false;
function queueLayout() {
  if (layoutQueued) return;
  layoutQueued = true;
  requestAnimationFrame(() => {
    layoutQueued = false;
    const r = dock.getBoundingClientRect();
    invoke("layout_dock", { width: Math.ceil(r.width), height: Math.ceil(r.height) }).catch(console.error);
  });
}
new ResizeObserver(queueLayout).observe(dock);

// ---------------------------------------------------------------------------
// Uygulamalar (sabitlenenler + açık olanlar)
// ---------------------------------------------------------------------------
const iconCache = new Map();
function iconFor(path) {
  const key = path.toLowerCase();
  if (!iconCache.has(key)) iconCache.set(key, invoke("app_icon", { path }).catch(() => null));
  return iconCache.get(key);
}

const baseName = (p) => p.split(/[\\/]/).pop().replace(/\.exe$/i, "");

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
    try { data = await invoke("list_windows"); } catch { return; }
    const groups = new Map();
    const own = cfg.display?.apps_per_monitor
      ? data.windows.filter((w) => !info.monitor || w.monitor === info.monitor)
      : data.windows;
    for (const w of own) {
      const k = w.exe.toLowerCase();
      if (!groups.has(k)) groups.set(k, []);
      groups.get(k).push(w);
    }
    const next = [];
    const pinnedKeys = new Set();
    for (const p of cfg.pinned_resolved.filter((p) => p.exists)) {
      const k = p.path.toLowerCase();
      pinnedKeys.add(k);
      next.push({ key: k, name: p.name, path: p.path, args: p.args, wins: groups.get(k) || [] });
    }
    if (cfg.show_running) {
      for (const [k, wins] of groups) {
        if (pinnedKeys.has(k)) continue;
        next.push({ key: k, name: baseName(wins[0].exe), path: wins[0].exe, args: null, wins });
      }
    }
    items = next;
    const activeKey = items.find((i) => i.wins.some((w) => w.hwnd === data.active))?.key || "";
    const sig = items.map((i) => `${i.key}:${i.wins.length}`).join("|") + "#" + activeKey;
    if (sig === lastSig) return;
    lastSig = sig;
    render(data.active);
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
      iconFor(item.path).then((url) => {
        if (!url) return;
        const img = new Image();
        img.src = url;
        img.alt = "";
        glyph.replaceWith(img);
      });
      if (item.wins.length) b.append($("span", "run"));

      b.onclick = () => {
        if (!item.wins.length) return invoke("launch_app", { path: item.path, args: item.args });
        // Birden çok pencere varsa ve biri öndeyse sıradakine geç.
        const i = item.wins.findIndex((w) => w.hwnd === active);
        const target = item.wins.length > 1 && i >= 0 ? item.wins[(i + 1) % item.wins.length] : item.wins[0];
        invoke("activate_window", { hwnd: target.hwnd }).then(() => setTimeout(refresh, 150));
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
  const c = $("div", "cell w-calendar");
  c.innerHTML = `
    <span class="datebox"><span class="dow"></span><span class="day"></span></span>
    <span class="stack"><span class="t-strong"></span><span class="t-sub">Takvim bağlantısı yakında</span></span>`;
  const [dow, day, title] = [c.querySelector(".dow"), c.querySelector(".day"), c.querySelector(".t-strong")];
  every(60_000, () => {
    const d = new Date();
    dow.textContent = trDate(d, { weekday: "short" }).toLocaleUpperCase("tr-TR").slice(0, 3);
    day.textContent = d.getDate();
    title.textContent = trDate(d, { day: "numeric", month: "long", weekday: "long" });
  });
  return c;
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

function screenTimeWidget() {
  const c = $("div", "cell w-screen");
  c.style.paddingLeft = "12px";
  c.innerHTML = `
    <span style="color:var(--chart);display:flex">${ICON.clock}</span>
    <span class="stack"><span class="t-sub">Ekran süresi</span><span class="t-big"></span></span>
    <span class="bars" aria-hidden="true"></span>`;
  const big = c.querySelector(".t-big");
  const bars = c.querySelector(".bars");
  every(30_000, async () => {
    let st;
    try { st = await invoke("screen_time"); } catch { return; }
    const h = Math.floor(st.seconds / 3600);
    const m = Math.floor((st.seconds % 3600) / 60);
    big.textContent = h ? `${h}s ${m}dk` : `${m}dk`;
    const now = new Date().getHours();
    bars.replaceChildren();
    for (let i = 6; i >= 0; i--) {
      const hour = now - i;
      const secs = hour >= 0 ? st.hours[hour] || 0 : 0;
      const s = $("span", i === 0 ? "" : "dim");
      s.style.height = `${Math.max(10, Math.min(100, (secs / 3600) * 100))}%`;
      bars.append(s);
    }
  });
  return c;
}

// ---------------------------------------------------------------------------
// Piyasa ve haberler: dolar, euro, gram altın ve haber başlıkları dönüşümlü
// ---------------------------------------------------------------------------
const MARKETS = {
  USD: { name: "Dolar", pair: "USD/TRY", glyph: "$", bg: "#1E9E63" },
  EUR: { name: "Euro", pair: "EUR/TRY", glyph: "€", bg: "#2F7BFF" },
  GBP: { name: "Sterlin", pair: "GBP/TRY", glyph: "£", bg: "#7C5CFF" },
  CHF: { name: "İsviçre frangı", pair: "CHF/TRY", glyph: "₣", bg: "#E5484D" },
  "gram-altin": { name: "Gram altın", pair: "TL", glyph: "g", bg: "linear-gradient(135deg,#F6C744,#B7791F)" },
  "ceyrek-altin": { name: "Çeyrek altın", pair: "TL", glyph: "¼", bg: "linear-gradient(135deg,#F6C744,#B7791F)" },
};
const NEWS_ICON = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 5h13v14H6a2 2 0 0 1-2-2z"/><path d="M17 9h3v8a2 2 0 0 1-2 2"/><path d="M8 9h5M8 13h5"/></svg>';

const trNum = (s) => Number(String(s).replace(/[%\s]/g, "").replace(/\./g, "").replace(",", "."));
const fmt2 = (n) => n.toLocaleString("tr-TR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });

function timeAgo(d) {
  const m = Math.round((Date.now() - d) / 60000);
  if (!isFinite(m) || m < 0) return "";
  if (m < 1) return "şimdi";
  if (m < 60) return `${m} dk önce`;
  const h = Math.round(m / 60);
  return h < 24 ? `${h} sa önce` : `${Math.round(h / 24)} gün önce`;
}

function parseFeed(xml, fallbackSource) {
  const doc = new DOMParser().parseFromString(xml, "text/xml");
  const txt = (el, sel) => el.querySelector(sel)?.textContent?.trim() || "";
  const channel = txt(doc, "channel > title") || txt(doc, "feed > title") || fallbackSource;
  const items = [...doc.querySelectorAll("item, entry")].map((it) => {
    const source = txt(it, "source") || channel;
    let title = txt(it, "title");
    if (source && title.endsWith(` - ${source}`)) title = title.slice(0, -(source.length + 3));
    const link = txt(it, "link") || it.querySelector("link")?.getAttribute("href") || "";
    const date = new Date(txt(it, "pubDate") || txt(it, "published") || txt(it, "updated"));
    return { kind: "news", title, link, source, date };
  });
  return items.filter((i) => i.title);
}

function tickerWidget() {
  const t = cfg.ticker || { markets: ["USD", "EUR", "gram-altin"], news: true, feeds: [], interval_sec: 6, news_per_round: 3 };
  const c = $("div", "cell ticker");
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
      el.title = `${item.title}\n${item.source}${ago}\nAçmak için tıkla`;
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
  c.onclick = () => {
    if (current?.kind === "news" && current.link) invoke("launch_app", { path: current.link });
    else advance();
  };

  show(null);
  (async () => {
    await Promise.all([loadMarkets(), loadNews()]);
    queue = [];
    advance();
  })();
  timers.push(setInterval(() => !paused && advance(), Math.max(3, t.interval_sec || 6) * 1000));
  timers.push(setInterval(loadMarkets, 5 * 60_000));
  timers.push(setInterval(loadNews, 15 * 60_000));
  return c;
}

function mediaWidget() {
  const c = $("div", "cell media idle");
  c.innerHTML = `
    <span class="art" aria-hidden="true">${ICON.wave}</span>
    <span class="stack meta"><span class="t-strong">Çalan bir şey yok</span><span class="t-sub"></span></span>
    <button class="ctrl" data-a="prev" aria-label="Önceki">${ICON.prev}</button>
    <button class="ctrl main" data-a="toggle" aria-label="Oynat/Duraklat">${ICON.play}</button>
    <button class="ctrl" data-a="next" aria-label="Sonraki">${ICON.next}</button>
    <span class="progress" aria-hidden="true"><span></span></span>`;
  const art = c.querySelector(".art");
  const [title, artist] = [c.querySelector(".t-strong"), c.querySelector(".t-sub")];
  const toggle = c.querySelector(".ctrl.main");
  const bar = c.querySelector(".progress > span");
  let trackKey = "";

  c.querySelectorAll(".ctrl").forEach((b) => {
    b.onclick = async () => {
      await invoke("media_control", { action: b.dataset.a });
      setTimeout(poll, 250);
    };
  });

  async function poll() {
    let m;
    try { m = await invoke("media_state"); } catch { return; }
    c.classList.toggle("idle", !m.active);
    if (!m.active) {
      title.textContent = "Çalan bir şey yok";
      artist.textContent = "";
      art.style.backgroundImage = "";
      art.classList.remove("has-img");
      trackKey = "";
      return;
    }
    title.textContent = m.title || "Bilinmeyen parça";
    artist.textContent = m.artist || m.source;
    c.title = [m.title, m.artist, m.album].filter(Boolean).join(" — ");
    toggle.innerHTML = m.playing ? ICON.pause : ICON.play;
    bar.style.width = m.duration_ms > 0 ? `${(m.position_ms / m.duration_ms) * 100}%` : "0%";
    if (m.track_key !== trackKey) {
      trackKey = m.track_key;
      const url = await invoke("media_thumbnail", { trackKey }).catch(() => null);
      art.style.backgroundImage = url ? `url("${url}")` : "";
      art.classList.toggle("has-img", !!url);
    }
  }
  every(1000, whenVisible(poll));
  return c;
}

function clockWidget() {
  const c = $("div", "cell plain clock");
  c.innerHTML = `<span class="stack"><span class="t-big"></span><span class="t-sub"></span></span>`;
  const [time, date] = [c.querySelector(".t-big"), c.querySelector(".t-sub")];
  every(5_000, () => {
    const d = new Date();
    time.textContent = d.toLocaleTimeString("tr-TR", { hour: "2-digit", minute: "2-digit" });
    date.textContent = trDate(d, { day: "numeric", month: "short", year: "numeric" });
  });
  return c;
}

const SYS_ICON = {
  tray: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 15l6-6 6 6"/></svg>',
  quick: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M2 8.5a15 15 0 0 1 20 0M5 12a10 10 0 0 1 14 0M8.5 15.5a5 5 0 0 1 7 0"/><circle cx="12" cy="19" r="1" fill="currentColor"/></svg>',
  notifications: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/></svg>',
  taskbar: '<svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="2.5"/><path d="M3 15.5h18M7 18h2M11 18h2"/></svg>',
};

function systemWidget() {
  const c = $("div", "cell sys");
  const items = [
    ["tray", "Gizli simgeler (sistem tepsisi)"],
    ["quick", "Hızlı ayarlar: Wi-Fi, ses, parlaklık (Win+A)"],
    ["notifications", "Bildirimler ve takvim (Win+N)"],
    ["taskbar", "Klasik taskbar'ı birkaç saniye göster"],
  ];
  for (const [action, label] of items) {
    const b = $("button", "", SYS_ICON[action]);
    b.title = label;
    b.setAttribute("aria-label", label);
    b.onclick = () => invoke("system_action", { action });
    c.append(b);
  }
  return c;
}

const WIDGETS = {
  system: systemWidget,
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
}

dock.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  invoke("show_menu").catch(console.error);
});

// Ekran eklenip çıkarılınca ya da ayar değişince yeniden yerleş.
listen("relayout", () => queueLayout());
listen("dock-visibility", (e) => { dockVisible = !!e.payload; });

listen("config-changed", async () => {
  cfg = await invoke("get_config");
  iconCache.clear();
  applyTheme();
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
        widgets: (params.get("widgets") || "calendar,screentime,ticker,media,system,clock").split(","),
        ticker: { markets: ["USD", "EUR", "gram-altin"], news: true, feeds: ["https://news.google.com/rss"], interval_sec: Number(params.get("iv") || 6), news_per_round: 3 },
        pinned_resolved: ["Dosya Gezgini", "Chrome", "VS Code", "Spotify", "Telegram"].map((n) => ({
          name: n, path: `C:\\${n}.exe`, args: null, exists: true,
        })),
      };
    case "list_windows":
      return {
        active: 2,
        windows: [
          { hwnd: 1, title: "Belgeler", exe: "C:\\Dosya Gezgini.exe", minimized: false },
          { hwnd: 2, title: "Cam Dock — Visual Studio Code", exe: "C:\\VS Code.exe", minimized: false },
          { hwnd: 3, title: "Spotify Premium", exe: "C:\\Spotify.exe", minimized: false },
        ],
      };
    case "screen_time":
      return { seconds: 6 * 3600 + 5 * 60, hours: [0,0,0,0,0,0,0,0,900,2400,3300,3000,1500,3600,2800,3400,2000,2600,0,0,0,0,0,0] };
    case "media_state":
      return { active: true, title: "Gece Sürüşü", artist: "Örnek Sanatçı", album: "", playing: true, position_ms: 134000, duration_ms: 218000, source: "Spotify.exe", track_key: "x" };
    case "fetch_text":
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
