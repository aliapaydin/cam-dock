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
const MARKET_OPTIONS = [
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
  { id: "screentime", name: "Ekran süresi ve kaynaklar", desc: "Aktif süre ile CPU, RAM, GPU, disk dönüşümlü; tıklayınca grafikli kullanım paneli." },
  { id: "ticker", name: "Piyasa ve haberler", desc: "Döviz, altın ve haber başlıkları dönüşümlü; tıklayınca kategorili haber paneli." },
  { id: "media", name: "Müzik", desc: "Spotify, YouTube ve diğer oynatıcılar; birden çoksa tekerlekle kaydır." },
  { id: "stacks", name: "Klasör yığınları", desc: "İndirilenler gibi klasörler; tıklayınca son dosyalar ızgara ya da yelpaze halinde." },
  { id: "live", name: "Canlı etkinlikler", desc: "İndirmeler, zamanlayıcı, mikrofon/kamera kullanımı, şarj; yalnızca bir şey sürerken görünür." },
  { id: "clipboard", name: "Pano geçmişi", desc: "Son kopyaladığın metin ve görseller; tıkla, tekrar kopyalansın." },
  { id: "system", name: "Sistem düğmeleri", desc: "Arka plan uygulamaları (tepsi), hızlı ayarlar, bildirimler, görev görünümü." },
  { id: "clock", name: "Saat ve hava durumu", desc: "Saat, tarih, sıcaklık; tıklayınca saatlik ve 6 günlük tahmin." },
  { id: "lock", name: "Kilit düğmesi", desc: "Dock'u kilitle (hep görünür) ya da otomatik gizle." },
  { id: "mail", name: "Mail", desc: "Önizleme — örnek veri.", preview: true },
  { id: "claude", name: "Claude kullanımı", desc: "5 saatlik ve haftalık limitler; Claude Code'da oturum açık olmalı." },
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
  for (const input of $$("input[type=time][data-bind]")) {
    input.onchange = () => { if (/^\d\d:\d\d$/.test(input.value)) set(input.dataset.bind, input.value); };
  }
  // Hava durumu: şehir arama (Open-Meteo geocoding)
  const searchCity = async () => {
    const q = $("#city-q").value.trim();
    const box = $("#city-res");
    if (q.length < 2) return;
    box.replaceChildren(el("span", "muted", "Aranıyor…"));
    try {
      const r = JSON.parse(await invoke("fetch_text", { url: `https://geocoding-api.open-meteo.com/v1/search?name=${encodeURIComponent(q)}&count=6&language=tr&format=json` }));
      const list = r.results || [];
      box.replaceChildren(...(list.length ? list.map((c) => chip(
        [c.name, c.admin1 && c.admin1 !== c.name ? c.admin1 : "", c.country_code !== "TR" ? c.country : ""].filter(Boolean).join(", "),
        false,
        () => {
          cfg.weather.city = c.name;
          cfg.weather.lat = c.latitude;
          cfg.weather.lon = c.longitude;
          cfg.weather.auto_location = false;
          box.replaceChildren();
          $("#city-q").value = "";
          changed();
        },
      )) : [el("span", "muted", "Bulunamadı")]));
    } catch {
      box.replaceChildren(el("span", "muted", "Arama yapılamadı"));
    }
  };
  $("#city-go").onclick = searchCity;
  $("#city-q").onkeydown = (e) => { if (e.key === "Enter") searchCity(); };
  // Güncellemeler
  $("#stack-add").onclick = async () => {
    const path = await invoke("pick_folder").catch(() => null);
    if (!path) return;
    cfg.stacks = cfg.stacks || [];
    if (cfg.stacks.some((x) => x.path.toLowerCase() === path.toLowerCase())) return;
    cfg.stacks.push({ name: path.split(/[\\/]/).filter(Boolean).pop(), path, view: "grid", sort: "date" });
    if (!cfg.widgets.includes("stacks")) cfg.widgets.unshift("stacks");
    renderStacks();
    changed();
  };
  $("#win-notif").onclick = () => invoke("launch_app", { path: "ms-settings:notifications" });
  let delArm = 0;
  $("#tel-delete").onclick = async () => {
    const b = $("#tel-delete");
    if (Date.now() - delArm > 4000) {
      delArm = Date.now();
      b.textContent = "Emin misin? Tekrar tıkla";
      setTimeout(() => { if (Date.now() - delArm >= 4000) b.textContent = "Verilerimi sil"; }, 4100);
      return;
    }
    b.disabled = true;
    b.textContent = "Siliniyor…";
    try {
      await invoke("telemetry_delete");
      cfg.telemetry.enabled = false;
      $("#tel-desc").textContent = "Verilerin silindi ve gönderim kapatıldı. İstersen yeniden açabilirsin; yeni bir kimlikle başlar.";
      b.textContent = "Silindi";
      refresh();
    } catch (e) {
      $("#tel-desc").textContent = "Silinemedi: sunucuya ulaşılamadı. İnternet bağlantını kontrol edip tekrar dene.";
      b.textContent = "Verilerimi sil";
    }
    b.disabled = false;
    delArm = 0;
  };
  $("#upd-check").onclick = () => invoke("update_check").catch(() => {});
  $("#upd-install").onclick = () => invoke("update_install").catch(() => {});
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
  $("#browse").onclick = async () => {
    const path = await invoke("pick_app").catch(() => null);
    if (!path) return;
    const name = path.split(/[\\/]/).pop().replace(/\.(exe|lnk)$/i, "");
    addPinned(name.charAt(0).toUpperCase() + name.slice(1), path);
  };
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
// Tema Rust'tan gelir (dinamik mod ve Windows teması dahil).
let effDark = matchMedia("(prefers-color-scheme: dark)").matches;
const refreshDark = () => invoke("theme_dark").then((d) => { effDark = !!d; if (cfg) refresh(); }).catch(() => {});
refreshDark();
listen("theme-dark", (e) => { effDark = !!e.payload; if (cfg) { Dyn.animate(document.documentElement); refresh(); } });
listen("location-changed", (e) => {
  if (!cfg) return;
  Object.assign(cfg.weather, { city: e.payload.city, lat: e.payload.lat, lon: e.payload.lon });
  refresh();
  loadSun();
});
Dyn.watch(() => cfg, () => refresh());

let sun = null;
const fmtMin = (m) => `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(Math.round(m % 60)).padStart(2, "0")}`;
const loadSun = () => invoke("sun_times").then((t) => { sun = t; if (cfg) refresh(); }).catch(() => {});
loadSun();

let upd = null;
invoke("update_status").then((u) => { upd = u; drawUpdate(); }).catch(() => {});
listen("update-status", (e) => { upd = e.payload; drawUpdate(); });
function drawUpdate() {
  if (!upd) return;
  const desc = {
    idle: "Henüz denetlenmedi.",
    checking: "Yeni sürüm denetleniyor…",
    uptodate: `Güncelsin.${upd.checked ? " Son denetim " + upd.checked : ""}`,
    downloading: `Sürüm ${upd.latest} indiriliyor… %${Math.round(upd.progress * 100)}`,
    ready: `Sürüm ${upd.latest} hazır. Bilgisayar boştayken kurulacak ya da şimdi kurabilirsin.`,
    installing: "Kuruluyor; dock birkaç saniye içinde yeniden açılacak…",
    error: upd.error || "Denetlenemedi.",
  }[upd.state] || "";
  $("#upd-title").textContent = `Cam Dock ${upd.current}`;
  $("#upd-desc").textContent = desc;
  $("#upd-bar").style.display = upd.state === "downloading" ? "block" : "none";
  $("#upd-bar i").style.width = `${Math.round(upd.progress * 100)}%`;
  $("#upd-install").style.display = upd.state === "ready" ? "" : "none";
  $("#upd-check").disabled = ["checking", "downloading", "installing"].includes(upd.state);
}

function refresh() {
  const root = document.documentElement;
  root.dataset.theme = effDark ? "dark" : "light";
  root.style.setProperty("--accent", cfg.accent);
  Dyn.apply(root, cfg, effDark, "settings").then(drawPalette);

  const dyn = cfg.dynamic;
  $(".dyn-card").classList.toggle("on", dyn.enabled);
  $("#tint-val").textContent = dyn.tint <= 15 ? "Çok hafif" : dyn.tint <= 40 ? "Hafif" : dyn.tint <= 70 ? "Belirgin" : "Güçlü";
  $("#sched-row").classList.toggle("disabled", !dyn.auto_theme);
  $("#fixed-row").style.display = dyn.auto_theme && dyn.schedule === "fixed" ? "" : "none";
  $("#sched-desc").textContent = dyn.schedule === "fixed"
    ? `Açık ${dyn.light_at}, koyu ${dyn.dark_at}`
    : sun ? `${cfg.weather.city || "Konum"}: bugün gün doğumu ${fmtMin(sun[0])}, batımı ${fmtMin(sun[1])}` : "Konum hava durumu ayarından alınır.";
  for (const inp of $$("input[type=time][data-bind]")) if (document.activeElement !== inp) inp.value = get(inp.dataset.bind);
  $("#manual-theme").classList.toggle("dyn-dim", dyn.enabled);
  $("#loc-desc").textContent = cfg.weather.auto_location
    ? `İnternet bağlantısından bulunur${cfg.weather.city ? " · şu an " + cfg.weather.city : ""}`
    : `Elle seçildi: ${cfg.weather.city || "–"}`;
  $("#city-row").classList.toggle("disabled", !cfg.weather.enabled);
  const nt = cfg.notifications;
  for (const id of ["#nt-balloon", "#nt-bounce", "#nt-text", "#nt-sec"]) $(id).classList.toggle("disabled", !nt.enabled);
  $("#nt-text").classList.toggle("disabled", !nt.enabled || !nt.balloons);
  $("#nt-sec").classList.toggle("disabled", !nt.enabled || !nt.bounce);
  $("#nt-sec-val").textContent = `${nt.bounce_sec} saniye`;
  $("#clip-max-row").classList.toggle("disabled", !cfg.clipboard.enabled);
  $("#clip-max-val").textContent = `Son ${cfg.clipboard.max_items} öğe (görseller en çok 12)`;
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

  $("#mag-row").classList.toggle("disabled", !cfg.magnify);
  $("#mag-val").textContent = `En fazla %${Math.round(((cfg.magnify_scale || 1.4) - 1) * 100)} büyür`;
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
  renderStacks();
  renderPhone();
}

function drawPalette() {
  const p = Dyn.preview();
  const box = $("#dyn-pal");
  if (!box) return;
  if (!p || !cfg.dynamic.enabled) { box.replaceChildren(); return; }
  const accent = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim();
  box.innerHTML = p.gray
    ? `<i style="background:#8a8f99"></i><small>Renksiz duvar kâğıdı: vurgu rengin korunur</small>`
    : `<i style="background:hsl(${Math.round(p.hue)} ${Math.round(Math.min(p.sat, 0.85) * 100)}% 50%)"></i><i style="background:${accent}"></i>`;
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
  for (const [key, label] of MARKET_OPTIONS) {
    const on = cfg.ticker.markets.includes(key);
    m.append(chip(label, on, () => {
      // Sırayı listedeki sırada tut
      const next = on ? cfg.ticker.markets.filter((k) => k !== key) : [...cfg.ticker.markets, key];
      cfg.ticker.markets = MARKET_OPTIONS.map(([k]) => k).filter((k) => next.includes(k));
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
function icon(path, name, aumid) {
  const holder = el("span", "ph", esc((name || "?").slice(0, 1).toUpperCase()));
  const key = (aumid || path).toLowerCase();
  if (!aumid && path.toLowerCase().startsWith("shell:appsfolder\\")) aumid = path.slice(17).includes("!") ? path.slice(17) : null;
  if (!iconCache.has(key)) iconCache.set(key, invoke("app_icon", { path, aumid: aumid || null, hwnd: null }).catch(() => null));
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

// ---------------------------------------------------------------------------
// Telefon ekranı
// ---------------------------------------------------------------------------
const PH_PAGES = { home: "Ana ekran (saat, hava, müzik, performans)", media: "Müzik çalar", shortcuts: "Kısayol tuşları", stats: "Performans grafikleri" };
const PH_KIND = { app: "Uygulama", keys: "Tuşlar", action: "İşlem" };
const PH_ACTIONS = { mute: "Sessize al", lock: "Kilitle", playpause: "Oynat/duraklat" };
let phTimer = null;

function renderPhone() {
  if (!cfg.phone) return;
  const ph = cfg.phone;
  // Sayfalar: açık olanlar sırayla, kapalılar altta
  const pages = $("#ph-pages");
  pages.replaceChildren();
  const all = [...ph.pages, ...Object.keys(PH_PAGES).filter((k) => !ph.pages.includes(k))];
  all.forEach((k) => {
    const on = ph.pages.includes(k);
    const i = ph.pages.indexOf(k);
    const row = el("div", "item");
    row.append(el("div", "grow", `<strong>${esc(PH_PAGES[k] || k)}</strong>`));
    if (on) {
      const up = el("button", "icon-btn", "↑");
      up.setAttribute("aria-label", "Yukarı");
      up.disabled = i === 0;
      up.onclick = () => { [ph.pages[i - 1], ph.pages[i]] = [ph.pages[i], ph.pages[i - 1]]; renderPhone(); changed(); };
      const dn = el("button", "icon-btn", "↓");
      dn.setAttribute("aria-label", "Aşağı");
      dn.disabled = i === ph.pages.length - 1;
      dn.onclick = () => { [ph.pages[i + 1], ph.pages[i]] = [ph.pages[i], ph.pages[i + 1]]; renderPhone(); changed(); };
      row.append(up, dn);
    }
    const sw = el("label", "switch", `<input type="checkbox" ${on ? "checked" : ""}><span></span>`);
    sw.querySelector("input").onchange = (e) => {
      if (e.target.checked) ph.pages.push(k);
      else if (ph.pages.length > 1) ph.pages = ph.pages.filter((x) => x !== k);
      renderPhone();
      changed();
    };
    row.append(sw);
    pages.append(row);
  });
  // Kısayollar
  const sc = $("#ph-shortcuts");
  sc.replaceChildren();
  if (!ph.shortcuts.length) sc.append(el("div", "empty", "Kısayol yok."));
  ph.shortcuts.forEach((s, i) => {
    const row = el("div", "item");
    row.append(el("span", "emo", esc(s.icon || "⚡")));
    const val = s.kind === "action" ? (PH_ACTIONS[s.value] || s.value) : s.value;
    row.append(el("div", "grow", `<strong>${esc(s.label)}</strong><small>${esc(PH_KIND[s.kind] || s.kind)} · ${esc(val)}</small>`));
    const up = el("button", "icon-btn", "↑");
    up.disabled = i === 0;
    up.onclick = () => { [ph.shortcuts[i - 1], ph.shortcuts[i]] = [ph.shortcuts[i], ph.shortcuts[i - 1]]; renderPhone(); changed(); };
    const x = el("button", "icon-btn", SVG.x);
    x.setAttribute("aria-label", "Kaldır");
    x.onclick = () => { ph.shortcuts.splice(i, 1); renderPhone(); changed(); };
    row.append(up, x);
    sc.append(row);
  });
  refreshPhone();
}

async function refreshPhone() {
  clearTimeout(phTimer);
  if (!$('.page[data-page="phone"]')?.classList.contains("active")) return;
  phTimer = setTimeout(refreshPhone, 2000);
  const st = await invoke("phone_status").catch(() => null);
  if (!st) return;
  $("#ph-live").hidden = !st.enabled;
  $("#ph-pin").textContent = st.pin.replace(/(\d{3})(\d{3})/, "$1 $2");
  $("#ph-addr").textContent = st.enabled
    ? (st.listening ? `${st.pc} · ${st.ip || "?"}:${st.port}` : `${st.port} numaralı bağlantı noktası kullanılamıyor`)
    : "";
  if ($("#ph-qr").dataset.u !== st.apk_url) {
    $("#ph-qr").innerHTML = st.apk_qr;
    $("#ph-qr").dataset.u = st.apk_url;
  }
  $("#ph-apk-link").onclick = (e) => { e.preventDefault(); navigator.clipboard?.writeText(st.apk_url); e.target.textContent = "Kopyalandı"; };
  $("#ph-usb").textContent = st.enabled && cfg.phone.usb && st.usb ? `USB: ${st.usb}` : "";
  const box = $("#ph-devices");
  box.replaceChildren();
  if (!st.devices.length) box.append(el("div", "empty", "Henüz eşleşmiş telefon yok."));
  for (const d of st.devices) {
    const row = el("div", "item");
    const on = st.connected.includes(d.name);
    row.append(el("span", "emo", "📱"));
    const when = d.last_seen ? new Date(d.last_seen * 1000).toLocaleString("tr-TR", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "";
    row.append(el("div", "grow", `<strong>${esc(d.name)}</strong><small>${on ? "Bağlı" : when ? "Son bağlantı " + when : ""}</small>`));
    if (on) row.append(el("i", "on-dot"));
    const x = el("button", "icon-btn", SVG.x);
    x.setAttribute("aria-label", "Eşleşmeyi kaldır");
    x.onclick = async () => { await invoke("phone_forget", { name: d.name }); refreshPhone(); };
    row.append(x);
    box.append(row);
  }
}

function bindPhone() {
  $("#ph-renew").onclick = async () => { await invoke("phone_renew_pin"); refreshPhone(); };
  const kind = $("#ph-sc-kind");
  const value = $("#ph-sc-value");
  kind.onchange = () => {
    value.placeholder = { app: "ör. C:\\Program Files\\…\\uygulama.exe", keys: "ör. win+shift+s", action: "mute, lock ya da playpause" }[kind.value];
  };
  $("#ph-sc-add").onclick = () => {
    const label = $("#ph-sc-label").value.trim();
    const v = value.value.trim();
    if (!label || !v) return;
    cfg.phone.shortcuts.push({ label, icon: $("#ph-sc-icon").value.trim() || "⚡", kind: kind.value, value: v });
    $("#ph-sc-label").value = ""; value.value = ""; $("#ph-sc-icon").value = "";
    renderPhone();
    changed();
  };
  for (const t of $$(".tab")) t.addEventListener("click", () => setTimeout(refreshPhone, 50));
}

function renderStacks() {
  const list = $("#stacks");
  if (!list) return;
  list.replaceChildren();
  const stacks = cfg.stacks || [];
  if (!stacks.length) {
    list.append(el("div", "empty", "Henüz klasör yığını yok."));
    return;
  }
  stacks.forEach((st, i) => {
    const row = el("div", "item");
    row.append(el("span", "ico", '<svg viewBox="0 0 40 34" width="26" height="22"><path d="M3 7a4 4 0 0 1 4-4h8l4 4h14a4 4 0 0 1 4 4v16a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4z" fill="var(--accent)"/></svg>'));
    row.append(el("div", "grow", `<strong>${esc(st.name)}</strong><small>${esc(st.path)}</small>`));
    const seg = el("div", "seg small");
    for (const [v, label] of [["grid", "Izgara"], ["fan", "Yelpaze"]]) {
      const b = el("button", st.view === v ? "on" : "", label);
      b.onclick = () => { st.view = v; renderStacks(); changed(); };
      seg.append(b);
    }
    row.append(seg);
    const x = el("button", "icon-btn", SVG.x);
    x.setAttribute("aria-label", "Kaldır");
    x.onclick = () => { stacks.splice(i, 1); renderStacks(); changed(); };
    row.append(x);
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
  startApps = await invoke("catalog_apps").catch(() => []);
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
    row.append(icon(a.path, a.name, a.aumid));
    row.append(el("div", "grow", `<strong>${esc(a.name)}</strong><small>${esc(a.aumid ? "Microsoft Store uygulaması" : a.path)}</small>`));
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
  bindPhone();
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
    case "phone_status":
      return { enabled: true, listening: true, port: 47800, ip: "192.168.1.20", pc: "ALI-PC", pin: "482913", clients: 1, connected: ["OMIX O1 Icon"],
        devices: [{ name: "OMIX O1 Icon", last_seen: 1762540000 }, { name: "OMIX O1 Icon (USB)", last_seen: 1762440000 }], usb: "OMIX O1 Icon kablo ile bağlı.",
        apk_url: "https://example.com/x.apk", apk_qr: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#fff"/><rect x="1" y="1" width="3" height="3"/><rect x="6" y="1" width="3" height="3"/><rect x="1" y="6" width="3" height="3"/><rect x="5" y="5" width="1" height="1"/><rect x="7" y="7" width="2" height="2"/></svg>' };
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
        dynamic: { enabled: q.has("wall"), tint: 45, auto_theme: true, schedule: q.get("sched") || "sun", light_at: "07:00", dark_at: "19:30" },
        weather: { enabled: true, auto_location: true, city: "İzmir", lat: 38.42, lon: 27.14 },
        update: { auto_check: true, auto_install: true },
        autostart: true,
        explorer_menu: true,
        tray_menus: true,
        magnify: true, magnify_scale: 1.4,
        stacks: [{ name: "İndirilenler", path: "%USERPROFILE%\\Downloads", view: "grid", sort: "date" }],
        notifications: { enabled: true, balloons: true, bounce: true, bounce_sec: 10, show_text: true },
        clipboard: { enabled: true, max_items: 40 },
        telemetry: { enabled: true, install_id: "3f6c1b2a-0000-4000-8000-000000000000", notice_shown: true },
        phone: { enabled: true, usb: true, pages: ["home", "media", "shortcuts", "stats"], devices: [], shortcuts: [
          { label: "Görev Yöneticisi", icon: "📊", kind: "keys", value: "ctrl+shift+esc" },
          { label: "Ekran görüntüsü", icon: "✂️", kind: "keys", value: "win+shift+s" },
          { label: "Hesap makinesi", icon: "🧮", kind: "app", value: "calc.exe" },
          { label: "Kilitle", icon: "🔒", kind: "action", value: "lock" } ] },
      };
    case "theme_dark": return (q.get("theme") || "dark") === "dark";
    case "wallpaper": return MOCK.wallpaper(q.get("wall"));
    case "sun_times": return [458, 1094];
    case "update_status": return { state: q.get("upd") || "uptodate", current: "0.10.0", latest: "0.10.1", progress: 0.42, checked: "30.10 20:55", error: "" };
    case "list_monitors":
      return [
        { name: "\\\\.\\DISPLAY1", title: "Ekran 1", width: 2560, height: 1440, scale: 1.25, primary: true, rect: [0, 0, 2560, 1440] },
        { name: "\\\\.\\DISPLAY2", title: "Ekran 2", width: 1920, height: 1080, scale: 1, primary: false, rect: [2560, 200, 4480, 1280] },
      ];
    case "catalog_apps":
      return ["Excel", "Figma", "Notion", "Outlook", "Power BI Desktop", "Spotify", "Teams", "WhatsApp"].map((n, i) => ({ name: n, path: i % 3 === 2 ? `shell:AppsFolder\\${n}!App` : `C:\\Apps\\${n}.exe`, aumid: i % 3 === 2 ? `${n}!App` : "" }));
    default:
      return null;
  }
}
