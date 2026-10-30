// Cam Dock ayarları. Her değişiklik kısa bir gecikmeyle kaydedilip anında uygulanır.
"use strict";

const TAURI = window.__TAURI__;
const invoke = TAURI ? TAURI.core.invoke : mockInvoke;
const listen = TAURI ? TAURI.event.listen : async () => () => {};

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
const el = (tag, cls, html) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (html != null) e.innerHTML = html;
  return e;
};
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

const SVG = {
  up: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M6 15l6-6 6 6"/></svg>',
  down: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9l6 6 6-6"/></svg>',
  x: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>',
  plus: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>',
};

const ACCENTS = ["#2F7BFF", "#7C5CFF", "#14B8A6", "#F97316", "#EC4899"];
const MARKETS = [
  ["USD", "Dolar"], ["EUR", "Euro"], ["GBP", "Sterlin"], ["CHF", "İsviçre frangı"],
  ["gram-altin", "Gram altın"], ["ceyrek-altin", "Çeyrek altın"],
];
const FEEDS = [
  ["https://news.google.com/rss?hl=tr&gl=TR&ceid=TR:tr", "Google Haberler"],
  ["https://feeds.bbci.co.uk/turkce/rss.xml", "BBC Türkçe"],
  ["https://www.ntv.com.tr/gundem.rss", "NTV Gündem"],
];
const WIDGETS = [
  { id: "calendar", name: "Takvim", desc: "Bugünün tarihi. Etkinlikler Faz 2'de bağlanacak." },
  { id: "screentime", name: "Ekran süresi", desc: "Bugün bilgisayar başında geçen aktif süre." },
  { id: "ticker", name: "Piyasa ve haberler", desc: "Dolar, euro, gram altın ve haber başlıkları dönüşümlü." },
  { id: "media", name: "Müzik", desc: "Spotify, YouTube ve diğer oynatıcılar için kontroller." },
  { id: "system", name: "Sistem düğmeleri", desc: "Gizli simgeler, hızlı ayarlar, bildirimler ve klasik taskbar." },
  { id: "clock", name: "Saat", desc: "Saat ve tarih." },
  { id: "lock", name: "Kilit düğmesi", desc: "Dock'u kilitle (hep görünür) ya da otomatik gizle." },
  { id: "mail", name: "Mail", desc: "Önizleme — örnek veri.", preview: true },
  { id: "claude", name: "Claude kullanımı", desc: "Önizleme — örnek veri.", preview: true },
];

let cfg = null;
let view = null; // get_config yanıtı (pinned_resolved, config_path)
let monitors = [];
let startApps = null;

// ---------------------------------------------------------------------------
// Ayar okuma/yazma
// ---------------------------------------------------------------------------
const get = (path) => path.split(".").reduce((o, k) => o?.[k], cfg);
function set(path, value) {
  const keys = path.split(".");
  let o = cfg;
  for (const k of keys.slice(0, -1)) o = o[k];
  o[keys.at(-1)] = value;
  changed();
}

let saveTimer = null;
function changed() {
  refresh();
  clearTimeout(saveTimer);
  saveTimer = setTimeout(save, 300);
}

async function save() {
  const note = $("#saved");
  try {
    await invoke("save_config", { cfg });
    view = await invoke("get_config");
    renderPinned();
    note.textContent = "Değişiklikler uygulandı";
  } catch (e) {
    note.textContent = "Kaydedilemedi: " + e;
  }
  note.style.opacity = 1;
  clearTimeout(note._t);
  note._t = setTimeout(() => (note.style.opacity = 0), 2200);
}

// ---------------------------------------------------------------------------
// Bağlamalar
// ---------------------------------------------------------------------------
function bind() {
  // Tek seçimli düğme grupları
  for (const group of $$(".seg[data-bind], .radios[data-bind]")) {
    for (const b of $$("button[data-v]", group)) {
      b.onclick = () => set(group.dataset.bind, b.dataset.v);
    }
  }
  // "Her zaman görünür" = kilitli; diğerleri kilit açıkken kullanılacak mod.
  for (const b of $$(".choices[data-bind] button[data-v]")) {
    b.onclick = () => {
      if (b.dataset.v === "always") cfg.dock_locked = true;
      else { cfg.dock_visibility = b.dataset.v; cfg.dock_locked = false; }
      changed();
    };
  }
  for (const input of $$("input[type=checkbox][data-bind]")) {
    input.onchange = () => set(input.dataset.bind, input.checked);
  }
  for (const input of $$("input[type=range][data-bind]")) {
    input.oninput = () => set(input.dataset.bind, Number(input.value));
  }
  for (const input of $$("input[type=number][data-bind]")) {
    input.onchange = () => set(input.dataset.bind, Math.max(1, Number(input.value) || 1));
  }
  $("#feed-add").onclick = () => {
    const url = $("#feed-url").value.trim();
    if (!/^https:\/\//.test(url) || cfg.ticker.feeds.includes(url)) return;
    cfg.ticker.feeds.push(url);
    $("#feed-url").value = "";
    changed();
  };

  // Sekmeler
  for (const t of $$(".tab")) {
    t.onclick = () => {
      $$(".tab").forEach((x) => x.classList.toggle("active", x === t));
      $$(".page").forEach((p) => p.classList.toggle("active", p.dataset.page === t.dataset.tab));
      if (t.dataset.tab === "apps") loadStartApps();
      if (t.dataset.tab === "displays") loadMonitors();
    };
  }

  // Renk
  const sw = $("#swatches");
  for (const c of ACCENTS) {
    const b = el("button");
    b.style.background = c;
    b.dataset.c = c;
    b.setAttribute("aria-label", `Vurgu rengi ${c}`);
    b.onclick = () => set("accent", c);
    sw.append(b);
  }
  const custom = el("input");
  custom.type = "color";
  custom.setAttribute("aria-label", "Özel renk");
  custom.oninput = () => set("accent", custom.value.toUpperCase());
  sw.append(custom);

  // Uygulama arama ve elle ekleme
  $("#app-search").oninput = renderResults;
  $("#m-add").onclick = async () => {
    const name = $("#m-name").value.trim();
    let path = $("#m-path").value.trim().replace(/^"|"$/g, "");
    if (!path) return;
    path = await invoke("resolve_shortcut", { path }).catch(() => path);
    addPinned(name || path.split(/[\\/]/).pop().replace(/\.(exe|lnk)$/i, ""), path);
    $("#m-name").value = $("#m-path").value = "";
  };

  for (const b of $$(".link-btn[data-href]")) b.onclick = () => invoke("launch_app", { path: b.dataset.href });
  $("#open-log").onclick = () => invoke("launch_app", { path: "notepad.exe", args: `"${view.log_path}"` });
  $("#open-cfg").onclick = () => invoke("launch_app", { path: "notepad.exe", args: `"${view.config_path}"` });
}

// ---------------------------------------------------------------------------
// Ekranı güncelle
// ---------------------------------------------------------------------------
const darkQuery = matchMedia("(prefers-color-scheme: dark)");
darkQuery.addEventListener("change", () => cfg && refresh());

function refresh() {
  const root = document.documentElement;
  root.dataset.theme = cfg.theme === "dark" || (cfg.theme !== "light" && darkQuery.matches) ? "dark" : "light";
  root.style.setProperty("--accent", cfg.accent);
  root.style.setProperty("--pv-radius", `${Math.round(cfg.radius * 0.66)}px`);
  root.style.setProperty("--pv-margin", `${Math.max(4, cfg.bottom_margin)}px`);

  for (const group of $$("[data-bind] button[data-v]")) {
    const g = group.closest("[data-bind]");
    group.classList.toggle("on", String(get(g.dataset.bind)) === group.dataset.v);
  }
  for (const input of $$("input[type=checkbox][data-bind]")) input.checked = !!get(input.dataset.bind);
  for (const input of $$("input[type=range][data-bind], input[type=number][data-bind], select[data-bind]")) {
    if (document.activeElement !== input) input.value = get(input.dataset.bind);
  }
  $$("#swatches button").forEach((b) => b.classList.toggle("on", b.dataset.c.toLowerCase() === cfg.accent.toLowerCase()));
  const custom = $("#swatches input[type=color]");
  if (custom && document.activeElement !== custom) custom.value = cfg.accent;

  $("#radius-val").textContent = `${cfg.radius} px`;
  $("#margin-val").textContent = `${cfg.bottom_margin} px`;
  $("#delay-val").textContent = `${(cfg.hide_delay_ms / 1000).toLocaleString("tr-TR")} sn sonra gizlenir`;

  const effective = cfg.dock_locked ? "always" : cfg.dock_visibility === "always" ? "autohide" : cfg.dock_visibility;
  $$(".choices button[data-v]").forEach((b) => b.classList.toggle("on", b.dataset.v === effective));
  const always = effective === "always";
  $("#delay-row").classList.toggle("disabled", always);
  $("#reserve-row").classList.toggle("disabled", !always);
  $("#reserve-desc").textContent = always
    ? "Maximize edilen pencereler dock'un altına girmez."
    : "Yalnızca dock kilitliyken (\"Her zaman görünür\") geçerli.";

  $("#iv-val").textContent = `${cfg.ticker.interval_sec} saniye`;
  $("#npr-val").textContent = `${cfg.ticker.news_per_round} haber, sonra kurlar tekrar`;
  $("#feeds-row").classList.toggle("disabled", !cfg.ticker.news);
  renderChips();
  renderMonitors();
  renderWidgets();
}

function chip(label, on, onclick, removable) {
  const b = el("button", "chip" + (on ? " on" : ""));
  b.textContent = label;
  if (removable) b.append(el("span", "x", "✕"));
  b.setAttribute("aria-pressed", on ? "true" : "false");
  b.onclick = onclick;
  return b;
}

function renderChips() {
  const m = $("#markets");
  m.replaceChildren();
  for (const [key, label] of MARKETS) {
    const on = cfg.ticker.markets.includes(key);
    m.append(chip(label, on, () => {
      // Sırayı listedeki sırada tut
      const next = on ? cfg.ticker.markets.filter((k) => k !== key) : [...cfg.ticker.markets, key];
      cfg.ticker.markets = MARKETS.map(([k]) => k).filter((k) => next.includes(k));
      changed();
    }));
  }
  const f = $("#feeds");
  f.replaceChildren();
  const known = FEEDS.map(([u]) => u);
  for (const [url, label] of FEEDS) {
    const on = cfg.ticker.feeds.includes(url);
    f.append(chip(label, on, () => {
      cfg.ticker.feeds = on ? cfg.ticker.feeds.filter((u) => u !== url) : [...cfg.ticker.feeds, url];
      changed();
    }));
  }
  for (const url of cfg.ticker.feeds.filter((u) => !known.includes(u))) {
    let host = url;
    try { host = new URL(url).hostname.replace(/^www\./, ""); } catch {}
    f.append(chip(host, true, () => {
      cfg.ticker.feeds = cfg.ticker.feeds.filter((u) => u !== url);
      changed();
    }, true));
  }
}

// ---------------------------------------------------------------------------
// Ekranlar
// ---------------------------------------------------------------------------
async function loadMonitors() {
  monitors = await invoke("list_monitors").catch(() => []);
  renderMonitors();
}

function dockOn(m) {
  const d = cfg.display;
  if (d.mode === "all") return true;
  if (d.mode === "selected") {
    const any = monitors.some((x) => d.selected.includes(x.name));
    return any ? d.selected.includes(m.name) : m.primary;
  }
  return m.primary;
}

function renderMonitors() {
  const box = $("#monitors");
  box.replaceChildren();
  if (!monitors.length) {
    box.append(el("div", "empty", "Ekran bilgisi alınamadı."));
    return;
  }
  const minX = Math.min(...monitors.map((m) => m.rect[0]));
  const minY = Math.min(...monitors.map((m) => m.rect[1]));
  const maxX = Math.max(...monitors.map((m) => m.rect[2]));
  const maxY = Math.max(...monitors.map((m) => m.rect[3]));
  const W = box.clientWidth - 40, H = box.clientHeight - 40;
  const k = Math.min(W / (maxX - minX), H / (maxY - minY));
  const ox = (box.clientWidth - (maxX - minX) * k) / 2;
  const oy = (box.clientHeight - (maxY - minY) * k) / 2;

  for (const m of monitors) {
    const b = el("button", "mon clickable");
    if (dockOn(m)) b.classList.add("has-dock");
    Object.assign(b.style, {
      left: `${ox + (m.rect[0] - minX) * k + 3}px`,
      top: `${oy + (m.rect[1] - minY) * k + 3}px`,
      width: `${(m.rect[2] - m.rect[0]) * k - 6}px`,
      height: `${(m.rect[3] - m.rect[1]) * k - 6}px`,
    });
    b.innerHTML = `${m.primary ? '<span class="badge">ANA EKRAN</span>' : ""}
      <strong>${esc(m.title)}</strong><small>${m.width}×${m.height} · %${Math.round(m.scale * 100)}</small>
      <span class="mini-dock"></span>`;
    b.title = dockOn(m) ? "Dock bu ekranda — kaldırmak için tıkla" : "Bu ekrana dock ekle";
    b.onclick = () => toggleMonitor(m);
    box.append(b);
  }
}

function toggleMonitor(m) {
  const current = monitors.filter(dockOn).map((x) => x.name);
  let next = current.includes(m.name) ? current.filter((n) => n !== m.name) : [...current, m.name];
  if (!next.length) return; // en az bir ekranda dock kalsın
  cfg.display.selected = next;
  cfg.display.mode =
    next.length === monitors.length && monitors.length > 1 ? "all"
    : next.length === 1 && monitors.find((x) => x.name === next[0])?.primary ? "primary"
    : "selected";
  changed();
}

// ---------------------------------------------------------------------------
// Uygulamalar
// ---------------------------------------------------------------------------
const iconCache = new Map();
function icon(path, name) {
  const holder = el("span", "ph", esc((name || "?").slice(0, 1).toUpperCase()));
  const key = path.toLowerCase();
  if (!iconCache.has(key)) iconCache.set(key, invoke("app_icon", { path }).catch(() => null));
  iconCache.get(key).then((url) => {
    if (!url) return;
    const img = new Image();
    img.src = url;
    img.alt = "";
    holder.replaceWith(img);
  });
  return holder;
}

function renderPinned() {
  const list = $("#pinned");
  list.replaceChildren();
  if (!cfg.pinned.length) {
    list.append(el("div", "empty", "Henüz sabitlenen uygulama yok. Aşağıdan ekleyebilirsin."));
    return;
  }
  cfg.pinned.forEach((p, i) => {
    const row = el("div", "item");
    row.append(icon(p.path, p.name));
    const missing = view?.pinned_resolved?.[i] && !view.pinned_resolved[i].exists;
    row.append(el("div", "grow", `<strong>${esc(p.name)}</strong><small>${esc(p.path)}</small>`));
    if (missing) row.append(el("span", "tag warn", "Bulunamadı"));
    const btn = (svg, label, fn, disabled) => {
      const b = el("button", "icon-btn", svg);
      b.setAttribute("aria-label", label);
      b.disabled = !!disabled;
      b.onclick = fn;
      return b;
    };
    row.append(
      btn(SVG.up, "Yukarı taşı", () => move(cfg.pinned, i, -1), i === 0),
      btn(SVG.down, "Aşağı taşı", () => move(cfg.pinned, i, 1), i === cfg.pinned.length - 1),
      btn(SVG.x, "Kaldır", () => { cfg.pinned.splice(i, 1); renderPinned(); renderResults(); changed(); }),
    );
    list.append(row);
  });
}

function move(arr, i, d) {
  const j = i + d;
  if (j < 0 || j >= arr.length) return;
  [arr[i], arr[j]] = [arr[j], arr[i]];
  renderPinned();
  changed();
}

const isPinned = (path) => cfg.pinned.some((p) => p.path.toLowerCase() === path.toLowerCase());

function addPinned(name, path) {
  if (isPinned(path)) return;
  cfg.pinned.push({ name, path, args: null });
  renderPinned();
  renderResults();
  changed();
}

async function loadStartApps() {
  if (startApps) return;
  $("#results").replaceChildren(el("div", "empty", "Başlat menüsü taranıyor…"));
  startApps = await invoke("start_menu_apps").catch(() => []);
  renderResults();
}

function renderResults() {
  const box = $("#results");
  if (!startApps) return;
  const q = $("#app-search").value.trim().toLocaleLowerCase("tr-TR");
  const hits = startApps.filter((a) => !q || a.name.toLocaleLowerCase("tr-TR").includes(q)).slice(0, 60);
  box.replaceChildren();
  if (!hits.length) {
    box.append(el("div", "empty", q ? "Eşleşen uygulama yok." : "Başlat menüsünde uygulama bulunamadı."));
    return;
  }
  for (const a of hits) {
    const row = el("div", "item");
    row.append(icon(a.path, a.name));
    row.append(el("div", "grow", `<strong>${esc(a.name)}</strong><small>${esc(a.path)}</small>`));
    if (isPinned(a.path)) {
      row.append(el("span", "tag", "Sabitli"));
    } else {
      const b = el("button", "icon-btn", SVG.plus);
      b.setAttribute("aria-label", `${a.name} uygulamasını sabitle`);
      b.onclick = () => addPinned(a.name, a.path);
      row.append(b);
    }
    box.append(row);
  }
}

// ---------------------------------------------------------------------------
// Widget'lar
// ---------------------------------------------------------------------------
function renderWidgets() {
  const list = $("#widgets");
  list.replaceChildren();
  const on = cfg.widgets.filter((id) => WIDGETS.some((w) => w.id === id));
  const ordered = [...on.map((id) => WIDGETS.find((w) => w.id === id)), ...WIDGETS.filter((w) => !on.includes(w.id))];
  ordered.forEach((w) => {
    const idx = cfg.widgets.indexOf(w.id);
    const enabled = idx >= 0;
    const row = el("div", "item");
    const sw = el("label", "switch", `<input type="checkbox" ${enabled ? "checked" : ""} aria-label="${esc(w.name)}"><span></span>`);
    sw.querySelector("input").onchange = (e) => {
      if (e.target.checked) cfg.widgets.push(w.id);
      else cfg.widgets = cfg.widgets.filter((x) => x !== w.id);
      changed();
    };
    row.append(sw);
    row.append(el("div", "grow", `<strong>${esc(w.name)}</strong><small>${esc(w.desc)}</small>`));
    if (w.preview) row.append(el("span", "tag", "Önizleme"));
    const btn = (svg, label, d, disabled) => {
      const b = el("button", "icon-btn", svg);
      b.setAttribute("aria-label", label);
      b.disabled = disabled;
      b.onclick = () => {
        const j = idx + d;
        [cfg.widgets[idx], cfg.widgets[j]] = [cfg.widgets[j], cfg.widgets[idx]];
        changed();
      };
      return b;
    };
    row.append(
      btn(SVG.up, "Yukarı taşı", -1, !enabled || idx === 0),
      btn(SVG.down, "Aşağı taşı", 1, !enabled || idx === cfg.widgets.length - 1),
    );
    list.append(row);
  });
}

// ---------------------------------------------------------------------------
// Başlat
// ---------------------------------------------------------------------------
async function init() {
  view = await invoke("get_config");
  const { pinned_resolved, config_path, log_path, ...rest } = view;
  cfg = rest;
  $("#log-path").textContent = log_path || "";
  cfg.display ??= { mode: "primary", selected: [], apps_per_monitor: false };
  cfg.ticker ??= { markets: ["USD", "EUR", "gram-altin"], news: true, feeds: [FEEDS[0][0]], interval_sec: 6, news_per_round: 3 };
  cfg.widgets = cfg.widgets.map((w) => (w === "currency" ? "ticker" : w));
  $("#cfg-path").textContent = config_path;
  bind();
  refresh();
  renderPinned();
  loadMonitors();
  const tab = new URLSearchParams(location.search).get("tab");
  if (tab) $(`.tab[data-tab="${tab}"]`)?.click();
}

// Dock'un sağ tık menüsünden tema değişirse burada da göster.
listen("config-changed", async () => {
  view = await invoke("get_config");
  const { pinned_resolved, config_path, log_path, ...rest } = view;
  if (rest.theme !== cfg.theme) {
    cfg.theme = rest.theme;
    refresh();
  }
});

init();

// ---------------------------------------------------------------------------
// Tarayıcıda önizleme için sahte köprü
// ---------------------------------------------------------------------------
async function mockInvoke(cmd) {
  const q = new URLSearchParams(location.search);
  switch (cmd) {
    case "get_config":
      return {
        theme: q.get("theme") || "dark", backdrop: "acrylic", hide_taskbar: true, dock_visibility: q.get("vis") || "smart",
        hide_delay_ms: 600, hide_in_fullscreen: true, reserve_space: true, bottom_margin: 10, radius: 26,
        accent: "#2F7BFF", show_running: true,
        display: { mode: "all", selected: [], apps_per_monitor: true },
        pinned: [
          { name: "Dosya Gezgini", path: "C:\\Windows\\explorer.exe", args: null },
          { name: "Chrome", path: "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe", args: null },
          { name: "VS Code", path: "C:\\Users\\ali\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe", args: null },
          { name: "Spotify", path: "C:\\Users\\ali\\AppData\\Roaming\\Spotify\\Spotify.exe", args: null },
        ],
        pinned_resolved: [{ exists: true }, { exists: true }, { exists: true }, { exists: false }],
        widgets: ["calendar", "screentime", "ticker", "media", "system", "clock", "lock"],
        dock_locked: false,
        ticker: { markets: ["USD", "EUR", "gram-altin"], news: true, feeds: ["https://news.google.com/rss?hl=tr&gl=TR&ceid=TR:tr", "https://example.com/feed.xml"], interval_sec: 6, news_per_round: 3 },
        config_path: "C:\\Users\\ali\\AppData\\Roaming\\com.ali.camdock\\config.json",
      };
    case "list_monitors":
      return [
        { name: "\\\\.\\DISPLAY1", title: "Ekran 1", width: 2560, height: 1440, scale: 1.25, primary: true, rect: [0, 0, 2560, 1440] },
        { name: "\\\\.\\DISPLAY2", title: "Ekran 2", width: 1920, height: 1080, scale: 1, primary: false, rect: [2560, 200, 4480, 1280] },
      ];
    case "start_menu_apps":
      return ["Excel", "Figma", "Notion", "Outlook", "Power BI Desktop", "Slack", "Teams", "Word"].map((n) => ({ name: n, path: `C:\\Apps\\${n}.exe`, shortcut: "" }));
    default:
      return null;
  }
}
