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

let lastThemeSig = "";
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
  const sig = JSON.stringify([dark, tint]);
  if (sig !== lastThemeSig) {
    lastThemeSig = sig;
    invoke("set_theme", { dark, tint }).catch(() => {});
  }
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

// Pencerenin dock'un üstünde büyüteç için ayırdığı boşluk (css px). Boşluk
// normalde tıklamaları alttaki pencerelere geçirir; yalnızca fare simgelerin
// üstündeyken simgeler hizasında açılır (dock_region).
const HEAD = 46;
document.documentElement.style.setProperty("--head", HEAD + "px");
function sendLayout() {
  const r = dock.getBoundingClientRect();
  const size = `${Math.ceil(r.width)}x${Math.ceil(r.height)}`;
  if (size === lastSize) return;
  lastSize = size;
  invoke("layout_dock", { width: Math.ceil(r.width), height: Math.ceil(r.height), head: HEAD }).catch(console.error);
}
// Dock daralacaksa biraz bekle: bir pencere kısa süre kaybolup geri gelince
// (uygulama yeniden çizerken, açılış ekranı vb.) dock gidip gelmesin.
function queueLayout() {
  if (!layoutReady) return;
  clearTimeout(layoutTimer);
  const w = Math.ceil(dock.getBoundingClientRect().width);
  const last = Number(lastSize.split("x")[0]) || 0;
  layoutTimer = setTimeout(sendLayout, w < last ? 900 : 60);
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

// ---------------------------------------------------------------------------
// Bildirimler: rozet sayısı, zıplama, cam balon
// ---------------------------------------------------------------------------
let notifSnap = { ok: false, apps: {}, recent: [] };
let appsDragging = false;      // sürüklerken liste yeniden çizilmesin
let suppressClickUntil = 0;    // bırakınca tıklama sayılmasın
const notifSeen = new Map();   // dock öğe anahtarı → görülen en büyük bildirim kimliği
const bounceUntil = new Map(); // dock öğe anahtarı → zıplamanın biteceği an
let catalogNames = null;       // AUMID (küçük harf) → uygulama adı
const appsApi = { items: () => [], redraw: () => {}, anchorOf: () => null, activate: () => {} };

const lc = (s) => (s || "").toLowerCase();
/** Windows'un bildirim kimliği bu dock öğesine mi ait? */
function notifMatches(handler, item) {
  const h = lc(handler);
  if (!h) return false;
  if (item.aumid && h === lc(item.aumid)) return true;
  for (const w of item.wins || []) if (w.aumid && h === lc(w.aumid)) return true;
  if (lc(item.path).startsWith("shell:appsfolder\\") && h === lc(item.path).slice(17)) return true;
  // Masaüstü uygulamaları: "Microsoft.Office.OUTLOOK.EXE.15", "...\\chrome.exe" gibi
  const base = lc(item.path).split(/[\\/]/).pop().replace(/\.exe$/, "");
  if (base.length >= 4 && (h.includes(base + ".exe") || h.split(/[.!_\\/]/).includes(base))) return true;
  return false;
}
function itemForHandler(handler) {
  return appsApi.items().find((it) => notifMatches(handler, it)) || null;
}
/** Dock öğesinin rozet metni (yoksa ""). */
function badgeFor(item) {
  if (!notifSnap.ok || cfg.notifications?.enabled === false) return "";
  let badge = null, toasts = 0;
  const seen = notifSeen.get(item.key) || 0;
  for (const [handler, n] of Object.entries(notifSnap.apps || {})) {
    if (!notifMatches(handler, item)) continue;
    if (n.badge) badge = n.badge;
    toasts += n.toasts.filter((id) => id > seen).length;
  }
  if (badge) return badge;
  return toasts ? (toasts > 99 ? "99+" : String(toasts)) : "";
}
/** Kullanıcı uygulamayı açtı: o ana kadarki bildirimleri görülmüş say. */
function markSeen(item) {
  let max = notifSeen.get(item.key) || 0;
  for (const [handler, n] of Object.entries(notifSnap.apps || {})) {
    if (notifMatches(handler, item)) max = Math.max(max, ...n.toasts);
  }
  notifSeen.set(item.key, max);
}
function prettyHandler(h) {
  const tail = h.split("!").pop().split("\\").pop();
  const parts = tail.split(".").filter((p) => !/^(exe|\d+|microsoft|com|office)$/i.test(p));
  const name = parts.sort((a, b) => b.length - a.length)[0] || tail;
  return name.replace(/_[a-z0-9]{13}$/i, "").replace(/([a-z])([A-Z])/g, "$1 $2");
}
async function appInfoFor(handler) {
  const it = itemForHandler(handler);
  if (it) return { name: it.name, icon: await iconFor(it), item: it };
  if (!catalogNames) {
    catalogNames = new Map();
    const list = await invoke("catalog_apps").catch(() => []);
    for (const a of list || []) if (a.aumid) catalogNames.set(lc(a.aumid), a);
  }
  const c = catalogNames.get(lc(handler));
  const icon = await invoke("app_icon", { path: c?.path || "", aumid: handler, hwnd: null }).catch(() => null);
  return { name: c?.name || prettyHandler(handler), icon, item: null };
}

listen("notifications", async (e) => {
  const { snap, new: fresh } = e.payload || {};
  if (snap) notifSnap = snap;
  const nc = cfg?.notifications || {};
  for (const t of fresh || []) {
    const info = await appInfoFor(t.app);
    if (info.item && nc.bounce !== false) {
      bounceUntil.set(info.item.key, Date.now() + (nc.bounce_sec || 10) * 1000);
      setTimeout(() => appsApi.redraw(), (nc.bounce_sec || 10) * 1000 + 50);
    }
    if (nc.balloons !== false && dockVisible !== null) {
      const anchor = (info.item && appsApi.anchorOf(info.item.key)) ?? dock.getBoundingClientRect().width - 160;
      invoke("toast_show", {
        anchor,
        payload: {
          id: t.id,
          key: info.item?.key || "",
          handler: t.app,
          name: info.name,
          icon: info.icon,
          title: nc.show_text === false ? "Yeni bildirim" : t.title,
          body: nc.show_text === false ? "" : t.body,
          time: t.time,
        },
      }).catch(console.error);
    }
  }
  appsApi.redraw();
});
listen("pinned-added", (e) => {
  if (!info.primary) return;
  const names = e.payload || [];
  invoke("toast_show", {
    anchor: dock.getBoundingClientRect().width / 3,
    payload: { id: -Date.now(), key: "__pinned", handler: "", name: "Cam Dock", icon: null,
      title: names.length > 1 ? `${names.length} uygulama sabitlendi` : `${names[0]} dock'a sabitlendi`,
      body: names.length > 1 ? names.join(", ") : "", time: Date.now() },
  }).catch(() => {});
});
listen("apps-locked", (e) => {
  if (!cfg) return;
  cfg.apps_locked = !!e.payload;
  dock.classList.toggle("apps-locked", cfg.apps_locked);
  // Kilitlenince kısa bir "oturma" titreşimi
  dock.querySelectorAll(".apps .app").forEach((b, i) => {
    b.style.animationDelay = `${i * 25}ms`;
    b.classList.add("lock-nudge");
    setTimeout(() => { b.classList.remove("lock-nudge"); b.style.animationDelay = ""; }, 600 + i * 25);
  });
});
listen("toast-click", (e) => {
  if (e.payload === "__telemetry") return invoke("open_settings");
  const it = appsApi.items().find((x) => x.key === e.payload);
  if (it) appsApi.activate(it);
});
invoke("notif_state").then((s) => { if (s) { notifSnap = s; appsApi.redraw(); } }).catch(() => {});

function appsWidget() {
  const wrap = $("div", "apps");
  const start = $("button", "app start", `<span class="glyph">${ICON.start}</span>`);
  start.title = "Başlat";
  start.setAttribute("aria-label", "Başlat");
  start.onclick = () => invoke("open_start");
  wrap.append(start);

  // Çok uygulama varsa dock ekrana sığsın: simgeler kaydırılabilir bir
  // pencerede durur, fare kenarlara yaklaşınca yeni simgeler kayarak gelir.
  const vp = $("div", "apps-vp");
  const list = $("div", "apps");
  list.style.padding = "0";
  vp.append(list);
  wrap.append(vp);

  let lastSig = "";
  let lastActive = 0;
  const winSeen = new Map(); // hwnd → { w, hits, miss }
  let firstPoll = true;
  let items = [];

  async function refresh() {
    let data;
    try { data = await invoke("list_windows"); } catch { firstAppsDone(); return; }
    // Titremeyi önle: yeni bir pencere ancak iki ardışık sorguda görünürse
    // eklenir (öndeyse hemen), kaybolan pencere iki sorgu boyunca yoksa çıkar.
    const nowIds = new Set(data.windows.map((w) => w.hwnd));
    for (const w of data.windows) winSeen.set(w.hwnd, { w, hits: (winSeen.get(w.hwnd)?.hits || 0) + 1, miss: 0 });
    for (const [h, v] of winSeen) if (!nowIds.has(h)) { v.miss++; if (v.miss > 1) winSeen.delete(h); }
    data.windows = [...winSeen.values()]
      .filter((v) => firstPoll || v.hits >= 2 || v.w.hwnd === data.active)
      .map((v) => v.w);
    firstPoll = false;
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
      // AUMID büyük/küçük harfi korunmalı: küçük harfle Windows simgeyi bulamıyor,
      // genel pencere simgesi veriyordu (WhatsApp vb. Store uygulamaları).
      const realAumid = groups.get(k)?.find((w) => w.aumid)?.aumid || (aum ? p.path.slice(17) : "");
      next.push({ key: k, okey: "pin:" + pk, name: p.name, path: p.path, args: p.args, aumid: realAumid, wins: groups.get(k) || [], pinned: true });
    }
    if (cfg.show_running) {
      const running = [];
      for (const [k, wins] of groups) {
        if (used.has(k)) continue;
        const w0 = wins[0];
        const name = w0.aumid || isHost(w0.exe) ? (w0.title.split(" - ").pop() || w0.title) : baseName(w0.exe);
        if (!seenOrder.has(k)) seenOrder.set(k, ++seenCounter);
        running.push({ key: k, okey: k, name, path: w0.exe, args: null, aumid: w0.aumid, wins });
      }
      // Açık uygulamalar ilk göründükleri sırada kalır (öne gelen öne kaymaz).
      running.sort((a, b) => seenOrder.get(a.key) - seenOrder.get(b.key));
      next.push(...running);
    }
    // Elle düzenlenmiş sıra (sürükle-bırak): bilinenler o sırada, yeniler sonda.
    const order = cfg.app_order || [];
    if (order.length) {
      const rank = new Map(order.map((k, i) => [k, i]));
      const base = new Map(next.map((it, i) => [it, i]));
      next.sort((a, b) => (rank.get(a.okey) ?? order.length + base.get(a)) - (rank.get(b.okey) ?? order.length + base.get(b)));
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
    const sig = items.map((i) => `${i.key}:${i.wins.length}:${badgeFor(i)}`).join("|") + "#" + activeKey;
    lastActive = data.active;
    if (appsDragging) return firstAppsDone();
    if (sig === lastSig) return firstAppsDone();
    lastSig = sig;
    render(data.active);
    if (activeKey) requestAnimationFrame(() => ensureVisible(activeKey));
    firstAppsDone();
  }

  function render(active) {
    list.replaceChildren();
    requestAnimationFrame(() => list.dispatchEvent(new Event("magrefresh")));
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
      b.dataset.key = item.key;
      if (isActive) markSeen(item);
      const badge = badgeFor(item);
      if (badge) {
        const bd = $("span", "badge" + (badge === "•" ? " dot" : ""), badge === "•" ? "" : badge);
        b.append(bd);
        b.title += `\n${badge === "•" ? "Yeni bildirim" : badge + " bildirim"}`;
      }
      const until = bounceUntil.get(item.key) || 0;
      if (until > Date.now()) {
        b.classList.add("bounce");
        b.style.setProperty("--bounce-left", `${(until - Date.now()) / 1000}s`);
      }

      b.onclick = () => {
        if (Date.now() < suppressClickUntil) return;
        clearTimeout(hoverTimer);
        const sp = $("span", "splash");
        b.append(sp);
        setTimeout(() => sp.remove(), 650);
        markSeen(item);
        bounceUntil.delete(item.key);
        b.classList.remove("bounce");
        b.querySelector(".badge")?.remove();
        if (!item.wins.length) return invoke("launch_app", { path: item.path, args: item.args });
        // Öndeyse küçült, küçültülmüşse eski konumuna getir, arkadaysa öne getir.
        invoke("activate_app", { hwnds: item.wins.map((w) => w.hwnd) }).then(() => setTimeout(refresh, 300));
      };
      // Birden çok penceresi olan uygulamanın üstüne gelince tüm pencereleri göster.
      b.onmouseenter = () => {
        clearTimeout(hoverTimer);
        if (item.wins.length < 2 || appsDragging) return;
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

  // ---- Kaydırma (taşan simgeler) ----
  const SLOT = 54; // simge 50 + aralık 4
  let scroll = 0, scrollTarget = 0, maxScroll = 0, edgeV = 0, scrollRaf = 0, hovering = false;
  function fitViewport() {
    const n = list.children.length;
    const natural = Math.max(0, n * SLOT - 4);
    const dockW = dock.getBoundingClientRect().width;
    const vpW = vp.getBoundingClientRect().width - 28; // yan dolgular hariç
    const others = dockW - Math.max(0, vpW);
    const avail = (window.screen?.width || 1920) - 40 - others;
    let maxN = Math.max(4, Math.floor((avail + 4) / SLOT));
    if (cfg.apps_max_visible > 0) maxN = Math.min(maxN, cfg.apps_max_visible);
    const visN = Math.min(n, maxN);
    const w = Math.max(0, visN * SLOT - 4);
    vp.style.width = `${w + 28}px`;
    vp.classList.toggle("overflow", n > visN);
    maxScroll = Math.max(0, natural - w);
    scrollTarget = Math.min(Math.max(0, scrollTarget), maxScroll);
    tickScroll();
  }
  function tickScroll() {
    scrollRaf = 0;
    scrollTarget = Math.min(Math.max(0, scrollTarget + edgeV), maxScroll);
    scroll += (scrollTarget - scroll) * 0.22;
    if (Math.abs(scrollTarget - scroll) < 0.4) scroll = scrollTarget;
    list.style.transform = scroll ? `translateX(${-scroll.toFixed(1)}px)` : "";
    vp.classList.toggle("fade-l", scroll > 2);
    vp.classList.toggle("fade-r", scroll < maxScroll - 2);
    if (magX != null) magApply();
    if (scroll !== scrollTarget || edgeV) scrollRaf = requestAnimationFrame(tickScroll);
  }
  const kickScroll = () => { if (!scrollRaf) scrollRaf = requestAnimationFrame(tickScroll); };
  /** Etkin uygulama görünür alanın dışındaysa ona kaydır (fare dock'ta değilken). */
  function ensureVisible(key) {
    if (hovering || !maxScroll) return;
    const el = list.querySelector(`[data-key="${CSS.escape(key)}"]`);
    if (!el) return;
    const w = vp.getBoundingClientRect().width - 28;
    const left = [...list.children].indexOf(el) * SLOT;
    if (left < scrollTarget) scrollTarget = left;
    else if (left + 50 > scrollTarget + w) scrollTarget = left + 50 - w;
    kickScroll();
  }
  vp.addEventListener("wheel", (e) => {
    if (!maxScroll) return;
    e.preventDefault();
    scrollTarget += (Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY) * 0.8;
    kickScroll();
  }, { passive: false });

  // ---- Büyüteç (Mac gibi) ----
  // Fareye yakın simgeler büyür ve dock'un üstüne taşar; komşular kayarak yer
  // açar, uzaktakiler biraz küçülür. Toplam genişlik sabit kaldığı için dock
  // boyutu değişmez (titreme yok).
  let magRaf = 0, magX = null, regionTimer = null;
  const magItems = () => [...list.children];
  function magApply() {
    magRaf = 0;
    const els = magItems();
    if (magX == null || appsDragging || cfg.magnify === false || !els.length) {
      for (const b of els) if (!b.classList.contains("dragging") && !b.classList.contains("dropping")) b.style.transform = "";
      return;
    }
    const max = Math.min(1.8, Math.max(1.1, cfg.magnify_scale || 1.6));
    const lr = list.getBoundingClientRect();
    const n = els.length;
    // Düzen tekdüze (SLOT aralıklı); offsetLeft liste yerine üst öğeye göre olabilir
    const c = els.map((_, i) => lr.left + i * SLOT + 25);
    const R = SLOT * 1.55;
    const g = c.map((x) => Math.exp(-(((x - magX) / R) ** 2)));
    const sc = g.map((v) => 1 + (max - 1) * v);
    const sumS = sc.reduce((a, v) => a + v, 0);
    const sumF = sc.reduce((a, v, i) => a + v * (1 - g[i]), 0);
    const alpha = Math.min(0.24, Math.max(0, (sumS - n) / Math.max(sumF, 0.001)));
    const t = sc.map((v, i) => v * (1 - alpha * (1 - g[i])));
    const total = t.reduce((a, v) => a + v, 0) * SLOT;
    let acc = c[0] - SLOT / 2 + (n * SLOT - total) / 2;
    els.forEach((b, i) => {
      const nc = acc + (t[i] * SLOT) / 2;
      acc += t[i] * SLOT;
      // Büyüyen simge ayrıca biraz yükselir: dock'un üstüne belirgin taşar
      const lift = Math.max(0, t[i] - 1) * 16;
      b.style.transform = `translate(${(nc - c[i]).toFixed(1)}px, ${(-lift).toFixed(1)}px) scale(${t[i].toFixed(3)})`;
    });
  }
  vp.addEventListener("mouseenter", () => {
    hovering = true;
    clearTimeout(regionTimer);
    if (cfg.magnify === false) return;
    const r = vp.getBoundingClientRect();
    invoke("dock_region", { expand: [Math.max(0, r.left - 40), r.right + 40] }).catch(() => {});
  });
  vp.addEventListener("mousemove", (e) => {
    const r = vp.getBoundingClientRect();
    // Kenarlara yaklaşınca kaydır
    edgeV = 0;
    if (maxScroll && !appsDragging) {
      const zone = SLOT * 0.9, x = e.clientX - r.left - 14, w = r.width - 28;
      if (x < zone && scrollTarget > 0) edgeV = -(1 - Math.max(0, x) / zone) * 9;
      else if (x > w - zone && scrollTarget < maxScroll) edgeV = (1 - Math.max(0, w - x) / zone) * 9;
      if (edgeV) kickScroll();
    }
    if (cfg.magnify === false) return;
    list.classList.add("mag-on");
    magX = e.clientX;
    if (!magRaf) magRaf = requestAnimationFrame(magApply);
  });
  list.addEventListener("magrefresh", () => { fitViewport(); if (magX != null && !magRaf) magRaf = requestAnimationFrame(magApply); });
  vp.addEventListener("mouseleave", () => {
    hovering = false;
    edgeV = 0;
    magX = null;
    list.classList.remove("mag-on");
    if (!magRaf) magRaf = requestAnimationFrame(magApply);
    clearTimeout(regionTimer);
    // Simgeler küçülme animasyonunu bitirince boşluk yine tıklama geçirsin
    regionTimer = setTimeout(() => invoke("dock_region", { expand: null }).catch(() => {}), 320);
  });

  // ---- Sürükle-bırak ile sıralama ----
  // Simge farenin peşinden gider, aradaki simgeler yumuşakça kayar; bırakınca
  // yerine "oturur". Düzen kilitliyse (sağ tık → kaydet ve kilitle) taşınamaz.
  let drag = null;
  list.addEventListener("pointerdown", (e) => {
    const b = e.target.closest(".app");
    if (!b || e.button !== 0 || cfg.apps_locked) return;
    drag = { el: b, id: e.pointerId, x0: e.clientX, active: false };
  });
  function startDrag() {
    const els = [...list.children];
    if (els.length < 2) { drag = null; return; }
    drag.active = true;
    appsDragging = true;
    magX = null;
    edgeV = 0;
    for (const b of list.children) b.style.transform = "";
    clearTimeout(hoverTimer);
    invoke("popover_dock_hover", { hover: false }).catch(() => {});
    drag.els = els;
    drag.from = els.indexOf(drag.el);
    drag.to = drag.from;
    drag.rects = els.map((x) => x.getBoundingClientRect());
    drag.slot = drag.rects[1].left - drag.rects[0].left;
    drag.min = drag.rects[0].left - drag.rects[drag.from].left;
    drag.max = drag.rects[els.length - 1].left - drag.rects[drag.from].left;
    try { drag.el.setPointerCapture(drag.id); } catch {}
    list.classList.add("drag-on");
    drag.el.classList.add("dragging");
  }
  list.addEventListener("pointermove", (e) => {
    if (!drag || e.pointerId !== drag.id) return;
    const dx = e.clientX - drag.x0;
    if (!drag.active) {
      if (Math.abs(dx) < 6) return;
      startDrag();
      if (!drag) return;
    }
    // Uçlarda hafif direnç (lastik gibi)
    let x = dx;
    if (x < drag.min) x = drag.min - Math.sqrt(drag.min - x) * 2;
    if (x > drag.max) x = drag.max + Math.sqrt(x - drag.max) * 2;
    drag.el.style.transform = `translateX(${x}px) translateY(-3px) scale(1.12)`;
    const to = Math.max(0, Math.min(drag.els.length - 1, drag.from + Math.round(x / drag.slot)));
    if (to !== drag.to) {
      drag.to = to;
      drag.els.forEach((el, i) => {
        if (el === drag.el) return;
        let shift = 0;
        if (drag.from < to && i > drag.from && i <= to) shift = -drag.slot;
        if (drag.from > to && i >= to && i < drag.from) shift = drag.slot;
        el.style.transform = shift ? `translateX(${shift}px)` : "";
      });
    }
  });
  const endDrag = (e) => {
    if (!drag || (e && e.pointerId !== drag.id)) return;
    const d = drag;
    drag = null;
    if (!d.active) return;
    suppressClickUntil = Date.now() + 250;
    const final = (d.to - d.from) * d.slot;
    d.el.classList.remove("dragging");
    d.el.classList.add("dropping");
    d.el.style.transform = `translateX(${final}px)`;
    setTimeout(() => {
      // Sırayı uygula ve kaydet
      const [moved] = items.splice(d.from, 1);
      items.splice(d.to, 0, moved);
      cfg.app_order = items.map((it) => it.okey);
      invoke("set_app_order", { order: cfg.app_order }).catch(() => {});
      list.classList.remove("drag-on");
      appsDragging = false;
      lastSig = "";
      render(lastActive);
      const landed = list.querySelector(`[data-key="${CSS.escape(moved.key)}"]`);
      landed?.classList.add("landed");
      setTimeout(() => landed?.classList.remove("landed"), 520);
    }, 210);
  };
  list.addEventListener("pointerup", endDrag);
  list.addEventListener("pointercancel", endDrag);

  appsApi.items = () => items;
  appsApi.redraw = () => { lastSig = ""; render(lastActive); };
  appsApi.anchorOf = (key) => {
    const el = list.querySelector(`[data-key="${CSS.escape(key)}"]`);
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return r.left + r.width / 2;
  };
  appsApi.activate = (item) => {
    markSeen(item);
    bounceUntil.delete(item.key);
    if (!item.wins.length) return invoke("launch_app", { path: item.path, args: item.args });
    invoke("activate_window", { hwnd: item.wins[0].hwnd }).catch(() => invoke("activate_app", { hwnds: item.wins.map((w) => w.hwnd) }));
    setTimeout(() => appsApi.redraw(), 300);
  };

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
    c.classList.toggle("holiday", !!h);
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

// Claude kullanımı: Claude Code oturumundan 5 saatlik ve haftalık limitler.
let claudeCell = null;
function claudeWidget() {
  const c = $("button", "cell w-claude");
  c.innerHTML = `<span class="tile" style="background:#D97757">${ICON.spark}</span><span class="cl-body"></span>`;
  const body = c.querySelector(".cl-body");
  const tone = (p) => (p >= 85 ? "hi" : p >= 60 ? "mid" : "lo");
  const draw = (u) => {
    if (!u) return;
    const rows = (u.limits || []).slice(0, 3);
    c.classList.toggle("stale", u.state !== "ok" && rows.length > 0);
    if (!rows.length) {
      const msg = { loading: "Yükleniyor…", no_claude_code: "Claude Code oturumu yok", expired: "Oturum yenilenmeli", error: "Okunamadı" }[u.state] || "–";
      body.className = "cl-body stack";
      body.innerHTML = `<span class="t-strong">Claude kullanımı</span><span class="t-sub"></span>`;
      body.querySelector(".t-sub").textContent = msg;
    } else {
      body.className = "cl-body usage";
      body.innerHTML = rows.map((l) => `<span class="lbl"></span><span class="track ${tone(l.percent)}"><span style="width:${Math.min(100, l.percent)}%"></span></span><span class="val">%${Math.round(l.percent)}</span>`).join("");
      body.querySelectorAll(".lbl").forEach((e, i) => (e.textContent = rows[i].label));
    }
    c.title = `Claude${u.plan ? " " + u.plan : ""} kullanımı\n` +
      (u.limits || []).map((l) => `${l.label}: %${Math.round(l.percent)}`).join("\n") +
      (u.message ? `\n${u.message}` : "") + "\nAyrıntılar için tıkla";
  };
  invoke("claude_usage").then(draw).catch(() => {});
  listen("claude-usage", (e) => draw(e.payload));
  c.onclick = async () => {
    const r = c.getBoundingClientRect();
    const res = await invoke("popover_open", { kind: "claude", key: "claude", anchor: r.left + r.width / 2, payload: null }).catch(() => "");
    c.classList.toggle("active", res === "opened" || res === "open");
  };
  claudeCell = c;
  return c;
}

// Pano geçmişi düğmesi
let clipCell = null;
function clipboardWidget() {
  const c = $("button", "cell clipw");
  c.innerHTML = `<svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="6" y="4" width="12" height="17" rx="2.5"/><path d="M9.5 4V3.2c0-.66.54-1.2 1.2-1.2h2.6c.66 0 1.2.54 1.2 1.2V4"/><path d="M9.5 10h5M9.5 14h5M9.5 18h3"/></svg><span class="cnt"></span>`;
  c.setAttribute("aria-label", "Pano geçmişi");
  const cnt = c.querySelector(".cnt");
  const upd = () => invoke("clip_list").then((l) => {
    cnt.textContent = l?.length ? l.length : "";
    c.title = `Pano geçmişi · ${l?.length || 0} öğe\nTıkla: son kopyaladıklarını gör, tekrar kopyala`;
  }).catch(() => {});
  upd();
  listen("clip-changed", () => {
    upd();
    c.classList.remove("pulse");
    void c.offsetWidth;
    c.classList.add("pulse");
  });
  c.onclick = async () => {
    const r = c.getBoundingClientRect();
    const res = await invoke("popover_open", { kind: "clipboard", key: "clipboard", anchor: r.left + r.width / 2, payload: null }).catch(() => "");
    c.classList.toggle("active", res === "opened" || res === "open");
  };
  clipCell = c;
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

  const FX = { cpu: "var(--s1)", ram: "var(--s2)", gpu: "var(--s3)", disk: "var(--s4)" };
  function fill(el) {
    const page = el.dataset.page;
    // Arka plandaki ışık: kaynak sayfasında metriğin renginde, yük arttıkça güçlenir.
    if (page === "screen") {
      c.style.removeProperty("--fx1");
      c.style.removeProperty("--fx-o");
    } else if (stats) {
      c.style.setProperty("--fx1", FX[page]);
      c.style.setProperty("--fx-o", (0.14 + Math.max(0, stats[page]) / 100 * 0.32).toFixed(2));
    }
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
let mediaCell = null;
function mediaWidget() {
  const c = $("div", "cell media idle");
  c.innerHTML = `<div class="m-fx" aria-hidden="true"><i></i><i></i><span class="m-wave"></span><span class="m-wave w2"></span></div><div class="m-view"></div><div class="m-dots" aria-hidden="true"></div>`;
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
      // Çalarken arkadaki dalga kapak renklerini alır
      Dyn.artColors(url).then((col) => {
        if (shownId !== m.id) return;
        c.style.setProperty("--m-c1", col?.c1 || "var(--accent)");
        c.style.setProperty("--m-c2", col?.c2 || "color-mix(in oklch, var(--accent), #ff5fd2 50%)");
      });
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
      c.classList.remove("playing");
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
    c.classList.toggle("playing", !!pick.playing);
    renderDots();
  }

  c.addEventListener("wheel", (e) => {
    e.preventDefault();
    if (Math.abs(e.deltaY) < 4) return;
    step(e.deltaY > 0 ? 1 : -1);
  }, { passive: false });
  dots.onclick = (e) => { e.stopPropagation(); step(1); };
  c.title = "Birden çok oynatıcı varsa tıkla: diğerleri üstte listelenir (tekerlekle de geçebilirsin)";
  // Birden çok oynatıcı: tıklayınca diğerleri dock'un üstünde sıralanır.
  c.onclick = async () => {
    if (sessions.length < 2) return;
    const r = c.getBoundingClientRect();
    const res = await invoke("popover_open", { kind: "media", key: "media", anchor: r.left + r.width / 2, payload: { exclude: selected } }).catch(() => "");
    c.classList.toggle("active", res === "opened" || res === "open");
  };
  mediaCell = c;

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
    ["quick", "Hızlı ayarlar: Wi-Fi, Bluetooth, ses, parlaklık"],
    ["notifications", "Bildirimler (Win+N)"],
    ["taskview", "Görev görünümü: açık pencereler ve masaüstleri (Win+Tab)"],
  ];
  for (const [action, label] of items) {
    const b = $("button", "", SYS_ICON[action]);
    b.title = label;
    b.setAttribute("aria-label", label);
    b.onclick = () => {
      if (action === "tray" || action === "quick") {
        const r = b.getBoundingClientRect();
        invoke("popover_open", { kind: action, key: action, anchor: r.left + r.width / 2, payload: null });
      } else invoke("system_action", { action });
    };
    c.append(b);
  }
  return c;
}

// ---------------------------------------------------------------------------
// Canlı etkinlikler: indirme, zamanlayıcı, mikrofon/kamera, şarj
// Hiçbir şey yokken görünmez; bir etkinlik başlayınca dock'ta belirir.
// ---------------------------------------------------------------------------
const fmtBytes = (n) => {
  if (n < 1024 * 1024) return `${Math.max(1, Math.round(n / 1024))} KB`;
  if (n < 1024 ** 3) return `${(n / 1024 / 1024).toLocaleString("tr-TR", { maximumFractionDigits: 1 })} MB`;
  return `${(n / 1024 ** 3).toLocaleString("tr-TR", { maximumFractionDigits: 2 })} GB`;
};
const fmtClock = (ms) => {
  const s = Math.ceil(ms / 1000);
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = s % 60;
  return h ? `${h}:${String(m).padStart(2, "0")}:${String(x).padStart(2, "0")}` : `${m}:${String(x).padStart(2, "0")}`;
};
const LIVE_ICON = {
  dl: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v11M7 10l5 5 5-5M5 20h14"/></svg>',
  ok: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12l5 5 9-10"/></svg>',
  mic: '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3"/></svg>',
  cam: '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linejoin="round"><rect x="3" y="6" width="13" height="12" rx="2.5"/><path d="M16 10l5-3v10l-5-3z"/></svg>',
  bolt: '<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><path d="M13 2L4 14h7l-1 8 9-12h-7z"/></svg>',
};
let chimeCtx = null;
function chime() {
  try {
    chimeCtx = chimeCtx || new AudioContext();
    const t = chimeCtx.currentTime;
    [880, 1318.5, 1760].forEach((f, i) => {
      const o = chimeCtx.createOscillator(), g = chimeCtx.createGain();
      o.type = "sine"; o.frequency.value = f;
      g.gain.setValueAtTime(0.0001, t + i * 0.16);
      g.gain.exponentialRampToValueAtTime(0.18, t + i * 0.16 + 0.02);
      g.gain.exponentialRampToValueAtTime(0.0001, t + i * 0.16 + 0.9);
      o.connect(g).connect(chimeCtx.destination);
      o.start(t + i * 0.16); o.stop(t + i * 0.16 + 1);
    });
  } catch {}
}
listen("timer-done", (e) => {
  if (!info.primary) return;
  chime();
  invoke("toast_show", {
    anchor: dock.getBoundingClientRect().width / 2,
    payload: { id: -Date.now(), key: "__timer", handler: "", name: "Zamanlayıcı", icon: null,
      title: "Süre doldu", body: `${e.payload} tamamlandı.`, time: Date.now(), life: 12000 },
  }).catch(() => {});
});

function liveWidget() {
  const c = $("div", "cell live");
  c.hidden = true;
  const pills = new Map(); // anahtar → eleman
  let armedStop = 0;

  function pill(key, cls, html, onclick) {
    let el = pills.get(key);
    if (!el) {
      el = $("button", "lp " + cls);
      el.innerHTML = html;
      el.onclick = onclick;
      c.append(el);
      pills.set(key, el);
      el.classList.add("enter");
      requestAnimationFrame(() => requestAnimationFrame(() => el.classList.remove("enter")));
    }
    el.dataset.alive = "1";
    return el;
  }

  // İndirmeler tek, küçük bir kartta; birden çoksa satırlar dikey kayarak döner
  let dlIdx = 0, dlRot = 0, dlHold = 0;
  const DL_ROW = 34;
  function fmtEta(sec) {
    if (!isFinite(sec) || sec <= 0) return "";
    if (sec < 60) return "<1 dk";
    const m = Math.round(sec / 60);
    return m < 60 ? `${m} dk` : `${Math.floor(m / 60)} sa ${m % 60} dk`;
  }
  function renderDownloads(dls) {
    const el = pill("dl", "lp-dl", `
      <span class="lp-ic"><svg class="lp-ring" viewBox="0 0 36 36" width="32" height="32"><circle cx="18" cy="18" r="15" class="tr"/><circle cx="18" cy="18" r="15" class="tv"/></svg><span class="spin"></span><span class="ico"></span></span>
      <span class="dl-vp"><span class="dl-track"></span></span><i class="dl-count"></i>`, () => {
        const d = el._dls?.[dlIdx];
        if (d) invoke("launch_app", { path: "explorer.exe", args: `/select,"${d.path}"` });
      });
    if (!el._wheel) {
      el._wheel = true;
      el.addEventListener("wheel", (e) => {
        if ((el._dls?.length || 0) < 2) return;
        e.preventDefault();
        dlIdx = (dlIdx + (e.deltaY > 0 ? 1 : -1) + el._dls.length) % el._dls.length;
        dlHold = Date.now();
        paint();
      }, { passive: false });
      el.addEventListener("mouseenter", () => { dlHold = Date.now() + 1e9; });
      el.addEventListener("mouseleave", () => { dlHold = Date.now(); });
    }
    el._dls = dls;
    const track = el.querySelector(".dl-track");
    while (track.children.length < dls.length) track.append($("span", "dl-row"));
    while (track.children.length > dls.length) track.lastChild.remove();
    dls.forEach((d, i) => {
      const row = track.children[i];
      const pct = d.total > 0 ? Math.min(100, (d.bytes / d.total) * 100) : null;
      let sub;
      if (d.done) sub = `İndirildi · ${fmtBytes(d.bytes)}`;
      else if (d.total > 0) {
        const eta = d.speed > 1024 ? fmtEta((d.total - d.bytes) / d.speed) : "";
        sub = `%${Math.floor(pct)} · ${fmtBytes(d.total)}${eta ? ` · ${eta}` : ""}`;
      } else sub = `${fmtBytes(d.bytes)}${d.speed > 1024 ? ` · ${fmtBytes(d.speed)}/sn` : ""}`;
      row.innerHTML = `<b></b><small></small>`;
      row.querySelector("b").textContent = d.name;
      row.querySelector("small").textContent = sub;
      row.dataset.pct = pct == null ? "" : pct.toFixed(1);
      row.classList.toggle("done", !!d.done);
    });
    if (dlIdx >= dls.length) dlIdx = 0;
    // Birden fazlaysa 3,5 sn'de bir sıradakine kay (fare üstündeyken durur)
    if (dls.length > 1 && Date.now() - dlHold > 3500 && Date.now() - dlRot > 3500) {
      dlRot = Date.now();
      dlIdx = (dlIdx + 1) % dls.length;
    }
    paint();
    function paint() {
      const d = dls[dlIdx] || dls[0];
      track.style.transform = `translateY(${-dlIdx * DL_ROW}px)`;
      const cnt = el.querySelector(".dl-count");
      cnt.textContent = dls.length > 1 ? `${dlIdx + 1}/${dls.length}` : "";
      cnt.hidden = dls.length < 2;
      el.classList.toggle("multi", dls.length > 1);
      const pct = d.total > 0 ? Math.min(1, d.bytes / d.total) : null;
      const C = 2 * Math.PI * 15;
      const tv = el.querySelector(".tv");
      tv.setAttribute("stroke-dasharray", C);
      tv.setAttribute("stroke-dashoffset", C * (1 - (pct ?? 0)));
      el.classList.toggle("known", pct != null && !d.done);
      el.classList.toggle("done", !!d.done);
      el.querySelector(".ico").innerHTML = d.done ? LIVE_ICON.ok : LIVE_ICON.dl;
      el.title = dls.map((x) => `${x.name}${x.total ? ` — %${Math.round((x.bytes / x.total) * 100)}` : ""}`).join("\n") + "\nKlasörde göstermek için tıkla";
    }
  }

  async function poll() {
    const s = await invoke("live_state").catch(() => null);
    if (!s) return;
    for (const el of pills.values()) el.dataset.alive = "";

    if (s.timer) {
      const t = s.timer;
      const el = pill("timer", "lp-timer", `
        <svg class="lp-ring" viewBox="0 0 36 36" width="30" height="30"><circle cx="18" cy="18" r="15" class="tr"/><circle cx="18" cy="18" r="15" class="tv"/></svg>
        <span class="lp-tx"><b></b><small>Zamanlayıcı</small></span>`, () => {
          if (Date.now() - armedStop > 3000) { armedStop = Date.now(); el.querySelector("small").textContent = "Durdurmak için tekrar tıkla"; return; }
          invoke("live_timer", { minutes: null }); armedStop = 0;
        });
      const C = 2 * Math.PI * 15;
      el.querySelector(".tv").setAttribute("stroke-dasharray", C);
      el.querySelector(".tv").setAttribute("stroke-dashoffset", C * (1 - t.left_ms / t.total_ms));
      el.querySelector("b").textContent = fmtClock(t.left_ms);
      if (Date.now() - armedStop > 3000) el.querySelector("small").textContent = `Zamanlayıcı · ${t.label}`;
      el.classList.toggle("urgent", t.left_ms < 60000);
    }
    const dls = s.downloads || [];
    if (dls.length) renderDownloads(dls);

    if (s.mic?.length) {
      const el = pill("mic", "lp-mic", `<span class="lp-ic">${LIVE_ICON.mic}<i class="dot"></i></span><span class="lp-tx"><b>Mikrofon açık</b><small></small></span>`,
        () => invoke("launch_app", { path: "ms-settings:privacy-microphone" }));
      el.querySelector("small").textContent = s.mic.join(", ");
    }
    if (s.cam?.length) {
      const el = pill("cam", "lp-cam", `<span class="lp-ic">${LIVE_ICON.cam}<i class="dot"></i></span><span class="lp-tx"><b>Kamera açık</b><small></small></span>`,
        () => invoke("launch_app", { path: "ms-settings:privacy-webcam" }));
      el.querySelector("small").textContent = s.cam.join(", ");
    }
    if (s.charging != null) {
      const el = pill("chg", "lp-chg", `<span class="lp-ic">${LIVE_ICON.bolt}</span><span class="lp-tx"><b></b><small>Şarj oluyor</small></span>`, () => {});
      el.querySelector("b").textContent = `%${s.charging}`;
    }

    // Biten etkinlikler küçülerek çıksın
    for (const [k, el] of pills) {
      if (el.dataset.alive) continue;
      pills.delete(k);
      el.classList.add("leave");
      setTimeout(() => { el.remove(); if (!c.querySelector(".lp")) c.hidden = true; }, 320);
    }
    if (pills.size) c.hidden = false;
  }
  every(1000, whenVisible(poll));
  return c;
}

// ---------------------------------------------------------------------------
// Klasör yığınları: klasör simgesinin üstünde son dosyaların küçük resimleri;
// tıklayınca dosyalar ızgara ya da yelpaze halinde açılır.
// ---------------------------------------------------------------------------
const FOLDER_SVG = '<svg class="st-folder" viewBox="0 0 40 34" width="38" height="32" aria-hidden="true"><path d="M3 7a4 4 0 0 1 4-4h8l4 4h14a4 4 0 0 1 4 4v16a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4z" fill="var(--st-back)"/><path d="M3 12a3 3 0 0 1 3-3h28a3 3 0 0 1 3 3v15a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4z" fill="var(--st-front)"/></svg>';
let stacksCell = null;
function stacksWidget() {
  const wrap = $("div", "apps stacks");
  wrap.style.padding = "0 2px";
  for (const st of cfg.stacks || []) {
    const b = $("button", "app stack-btn");
    b.innerHTML = `<span class="st-ic">${FOLDER_SVG}<span class="st-thumbs"></span></span>`;
    b.title = `${st.name}\nSon dosyalar için tıkla`;
    b.setAttribute("aria-label", `${st.name} klasör yığını`);
    const thumbs = b.querySelector(".st-thumbs");
    const refresh = async () => {
      const list = (await invoke("stack_list", { path: st.path, sort: "date", limit: 3 }).catch(() => [])) || [];
      const urls = await Promise.all(list.slice(0, 3).map((e) => invoke("file_thumb", { path: e.path, size: 64 }).catch(() => null)));
      thumbs.replaceChildren(...urls.filter(Boolean).reverse().map((u, i, a) => {
        const img = new Image();
        img.src = u;
        img.alt = "";
        img.style.setProperty("--k", a.length - 1 - i);
        return img;
      }));
      b.classList.toggle("has-files", urls.some(Boolean));
    };
    refresh();
    timers.push(setInterval(whenVisible(refresh), 15000));
    b.onclick = async () => {
      const r = b.getBoundingClientRect();
      const res = await invoke("popover_open", { kind: "stack", key: st.path, anchor: r.left + r.width / 2, payload: st }).catch(() => "");
      b.classList.toggle("is-active", res === "opened" || res === "open");
      stacksCell = b;
      refresh();
    };
    wrap.append(b);
  }
  if (!(cfg.stacks || []).length) wrap.hidden = true;
  return wrap;
}

const WIDGETS = {
  system: systemWidget,
  lock: lockWidget,
  calendar: calendarWidget,
  mail: mailWidget,
  claude: claudeWidget,
  clipboard: clipboardWidget,
  live: liveWidget,
  stacks: stacksWidget,
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
  // Eski sürümden güncelleyenlere (kurulumda sorulmadı) istatistik için bir kez bilgi ver.
  if (info.primary && cfg.telemetry?.enabled && !cfg.telemetry.notice_shown) {
    setTimeout(() => {
      invoke("telemetry_notice_seen").catch(() => {});
      invoke("toast_show", {
        anchor: dock.getBoundingClientRect().width - 200,
        payload: {
          id: -1, key: "__telemetry", handler: "", name: "Cam Dock", icon: null,
          title: "Anonim kullanım istatistiği açık",
          body: "Sürüm, Windows sürümü ve ülke gibi anonim bilgiler gönderiliyor. Ayarlar → Hakkında'dan kapatabilirsin.",
          time: Date.now(),
          life: 15000,
        },
      }).catch(() => {});
    }, 8000);
  }
}

dock.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  invoke("show_menu").catch(console.error);
});

// Ekran eklenip çıkarılınca ya da ayar değişince yeniden yerleş.
listen("relayout", () => { lastSize = ""; queueLayout(); });
listen("dock-visibility", (e) => { dockVisible = !!e.payload; });
listen("popover-closed", (e) => {
  const cell = { calendar: calendarCell, usage: screenCell, news: tickerCell, weather: clockCell, claude: claudeCell, clipboard: clipCell, media: mediaCell }[e.payload];
  if (e.payload === "stack") stacksCell?.classList.remove("is-active");
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
        widgets: (params.get("widgets") || "stacks,live,calendar,screentime,ticker,media,clipboard,system,clock,lock").split(","),
        notifications: { enabled: true, balloons: true, bounce: true, bounce_sec: 10, show_text: true },
        dock_locked: params.get("locked") !== "0",
        stacks: [{ name: "İndirilenler", path: "%USERPROFILE%\\Downloads", view: "grid", sort: "date" }],
        magnify: true, magnify_scale: 1.4,
        dynamic: { enabled: params.has("wall"), tint: Number(params.get("tint") || 45), auto_theme: false, schedule: "sun", light_at: "07:00", dark_at: "19:30" },
        weather: { enabled: true, auto_location: true, city: "İstanbul", lat: 41, lon: 29 },
        ticker: { markets: ["USD", "EUR", "gram-altin"], news: true, feeds: ["https://news.google.com/rss"], interval_sec: Number(params.get("iv") || 6), news_per_round: 3 },
        pinned_resolved: (window.__MANY ? ["Dosya Gezgini", "Chrome", "Edge", "VS Code", "Spotify", "WhatsApp", "Claude", "Steam", "Epic", "Telegram", "Xbox", "Teams", "OneNote", "Outlook", "Excel", "Media"] : ["Dosya Gezgini", "Chrome", "VS Code", "Spotify", "Telegram"]).map((n) => ({
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
    case "claude_usage":
      return { state: "ok", plan: "Max", updated: Date.now() - 120000, message: "", limits: [
        { key: "session", label: "Oturum", percent: 34, resets_at: new Date(Date.now() + 2.3 * 3600e3).toISOString() },
        { key: "weekly_all", label: "Haftalık", percent: 58, resets_at: new Date(Date.now() + 3.4 * 86400e3).toISOString() },
        { key: "weekly_scoped:Opus", label: "Opus", percent: 87, resets_at: new Date(Date.now() + 3.4 * 86400e3).toISOString() },
      ] };
    case "clip_list":
      return new Array(7).fill(0);
    case "notif_state":
      return { ok: true, apps: { "C:\\Spotify.exe": { badge: null, toasts: [5, 6] }, "Microsoft.VSCode": { badge: "3", toasts: [] } }, recent: [] };
    case "stack_list":
      return ["Fatura-Ekim.pdf", "Ekran görüntüsü 2026-11-02.png", "Kurulum.exe", "Sunum.pptx", "Tatil.jpg", "Veri.xlsx", "Not.txt", "Arşiv.zip"].slice(0, args.limit || 60)
        .map((n, i) => ({ name: n, path: "C:\\Users\\x\\Downloads\\" + n, dir: false, modified: Date.now() - i * 3600e3 * 5, size: 1e6 * (i + 1) }));
    case "file_thumb":
      return null;
    case "live_state":
      return {
        downloads: [
          { name: "Unity Kits Megapack - Changes.zip", path: "C:\\Users\\x\\Downloads\\a.crdownload", bytes: 46_200_000, total: 139_000_000, speed: 90_000, done: false },
          { name: "Unity Kits Megapack (1).zip", path: "C:\\Users\\x\\Downloads\\b.crdownload", bytes: 48_900_000, total: 276_000_000, speed: 88_000, done: false },
          { name: "Kurulum.exe", path: "C:\\Users\\x\\Downloads\\c.crdownload", bytes: 2_100_000, total: 0, speed: 0, done: false },
        ],
        timer: { label: "25 dk", total_ms: 1500000, left_ms: 1103000 },
        mic: ["Teams"], cam: [], charging: null,
      };
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
