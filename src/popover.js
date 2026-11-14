// Dock'un üstündeki cam panel: pencere önizlemeleri ve takvim.
"use strict";

const TAURI = window.__TAURI__;
const invoke = TAURI ? TAURI.core.invoke : mockInvoke;
const listen = TAURI ? TAURI.event.listen : async () => () => {};

const panel = document.getElementById("panel");
const el = (tag, cls, html) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (html != null) e.innerHTML = html;
  return e;
};
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

const SVG = {
  prev: '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M15 18l-6-6 6-6"/></svg>',
  next: '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18l6-6-6-6"/></svg>',
  x: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>',
};

let serial = 0;
let spec = null;

// ---------------------------------------------------------------------------
// Tema (dock ile aynı)
// ---------------------------------------------------------------------------
let pcfg = null;
async function applyTheme(animate) {
  const cfg = await invoke("get_config").catch(() => null);
  if (!cfg) return;
  pcfg = cfg;
  const root = document.documentElement;
  const dark = !!(await invoke("theme_dark").catch(() => matchMedia("(prefers-color-scheme: dark)").matches));
  if (animate) Dyn.animate(root);
  root.dataset.theme = dark ? "dark" : "light";
  root.dataset.backdrop = cfg.backdrop;
  root.style.setProperty("--accent", cfg.accent || "#2F7BFF");
  await Dyn.apply(root, cfg, dark, "popover");
}
Dyn.watch(() => pcfg, () => applyTheme(true));

// ---------------------------------------------------------------------------
// Yerleşim
// ---------------------------------------------------------------------------
function relayout() {
  requestAnimationFrame(() => {
    // offsetWidth/Height: açılış animasyonunun küçültmesinden (scale .97)
    // etkilenmeyen gerçek boyut. getBoundingClientRect küçültülmüş ölçüyü
    // verdiği için pencere dar açılıyor, panelin sağı ve altı kesiliyordu.
    const w = Math.ceil(panel.offsetWidth) + 1, h = Math.ceil(panel.offsetHeight) + 1;
    invoke("popover_layout", { width: w, height: h, serial }).catch(console.error);
  });
}

async function show(s) {
  serial = s.serial;
  spec = s;
  panel.classList.remove("in");
  await applyTheme();
  stopLive();
  panel.replaceChildren();
  panel.dataset.kind = s.kind;
  if (s.kind === "calendar") renderCalendar();
  else if (s.kind === "tray") await renderTray();
  else if (s.kind === "usage") await renderUsage();
  else if (s.kind === "news") await renderNews(s.payload);
  else if (s.kind === "weather") await renderWeather();
  else if (s.kind === "clipboard") await renderClipboard();
  else if (s.kind === "media") await renderMedia(s.payload);
  else if (s.kind === "quick") await renderQuick();
  else if (s.kind === "stack") await renderStack(s.payload);
  else if (s.kind === "claude") await renderClaude();
  else renderWindows(s.payload);
  relayout();
}

listen("popover-show", (e) => show(e.payload));
listen("popover-in", (e) => {
  if (e.payload !== serial) return;
  requestAnimationFrame(() => panel.classList.add("in"));
  if (spec?.kind === "windows") setTimeout(sendThumbs, 260);
});
listen("popover-out", () => {
  stopLive();
  panel.classList.remove("in");
  setTimeout(() => invoke("popover_hidden").catch(() => {}), 200);
});
listen("config-changed", () => applyTheme(true));
listen("theme-dark", () => applyTheme(true));

panel.addEventListener("mouseenter", () => invoke("popover_hover", { hover: true }).catch(() => {}));
panel.addEventListener("mouseleave", () => invoke("popover_hover", { hover: false }).catch(() => {}));

// ---------------------------------------------------------------------------
// Pencere önizlemeleri
// ---------------------------------------------------------------------------
function renderWindows(p) {
  const head = el("div", "win-head");
  head.append(p.icon ? Object.assign(new Image(), { src: p.icon, alt: "" }) : el("span", "glyph", esc(p.name.slice(0, 1))));
  head.append(el("div", "", `<strong>${esc(p.name)}</strong> <span>· ${p.windows.length} pencere</span>`));
  panel.append(head);

  const cards = el("div", "cards");
  for (const w of p.windows) {
    const card = el("button", "card");
    card.dataset.hwnd = w.hwnd;
    card.setAttribute("aria-label", w.title);
    const thumb = el("div", "thumb");
    if (p.icon) thumb.append(Object.assign(new Image(), { src: p.icon, alt: "" }));
    if (w.minimized) thumb.append(el("span", "", "Küçültülmüş"));
    card.append(thumb);
    const row = el("div", "card-row");
    row.append(el("span", "title", esc(w.title)));
    const x = el("button", "x", SVG.x);
    x.title = "Pencereyi kapat";
    x.setAttribute("aria-label", `${w.title} penceresini kapat`);
    x.onclick = (e) => {
      e.stopPropagation();
      invoke("close_window", { hwnd: w.hwnd });
      card.remove();
      p.windows = p.windows.filter((o) => o.hwnd !== w.hwnd);
      if (!p.windows.length) return invoke("popover_close");
      head.querySelector("span").textContent = `· ${p.windows.length} pencere`;
      relayout();
      setTimeout(sendThumbs, 120);
    };
    row.append(x);
    card.append(row);
    card.onclick = () => invoke("popover_activate", { hwnd: w.hwnd });
    cards.append(card);
  }
  panel.append(cards);
}

function sendThumbs() {
  if (spec?.kind !== "windows") return;
  const items = [...panel.querySelectorAll(".card")].map((c) => {
    const r = c.querySelector(".thumb").getBoundingClientRect();
    return { hwnd: Number(c.dataset.hwnd), x: r.left + 4, y: r.top + 4, w: r.width - 8, h: r.height - 8 };
  });
  invoke("popover_thumbs", { items, serial }).catch(() => {});
}

// ---------------------------------------------------------------------------
// Sistem tepsisi: arka planda çalışan uygulamalar
// ---------------------------------------------------------------------------
async function renderTray() {
  const apps = await invoke("tray_apps").catch(() => []);
  const head = el("div", "tray-head");
  head.append(
    el("div", "tray-title", `<strong>Arka planda çalışanlar</strong><span class="tray-count">${apps.length}</span>`),
    el("span", "tray-hint", "Sağ tık: uygulamanın menüsü"),
  );
  panel.append(head);
  if (!apps.length) {
    panel.append(el("div", "tray-empty", "Tepside çalışan uygulama bulunamadı."));
  } else {
    const grid = el("div", "tray-grid");
    // Öne çıkarılmış (taskbar'da görünen) simgeler önce
    apps.sort((a, b) => Number(b.promoted) - Number(a.promoted));
    apps.forEach((a, i) => {
      const t = el("button", "tray-app");
      t.style.setProperty("--i", i);
      a.name = a.name.replace(/\.exe$/i, "");
      t.title = `${a.name}\n${a.exe}`;
      t.setAttribute("aria-label", a.name);
      const ic = el("span", "ic");
      // Önce uygulamanın kendi ikonu (tepsi görüntüsü koyu taskbar için beyaz
      // çizilmiş olabiliyor), yoksa Windows'un sakladığı tepsi simgesi.
      const glyph = a.icon ? Object.assign(new Image(), { src: a.icon, alt: "" }) : el("span", "glyph", esc(a.name.slice(0, 1).toUpperCase()));
      ic.append(glyph);
      t.append(ic);
      invoke("app_icon", { path: a.exe, aumid: null, hwnd: null }).then((url) => {
        if (url) glyph.replaceWith(Object.assign(new Image(), { src: url, alt: "" }));
      }).catch(() => {});
      t.append(el("span", "nm", esc(a.name)));
      if (a.promoted) t.classList.add("pro");
      const ripple = (e) => {
        const r = t.getBoundingClientRect();
        const w = el("i", "rip");
        w.style.left = `${e.clientX - r.left}px`;
        w.style.top = `${e.clientY - r.top}px`;
        t.append(w);
        setTimeout(() => w.remove(), 600);
      };
      t.onclick = (e) => {
        ripple(e);
        // Çoğu tepsi uygulaması tek örnekli: yeniden başlatmak penceresini açar.
        invoke("launch_app", { path: a.exe });
        setTimeout(() => invoke("popover_close"), 140);
      };
      // Sağ tık: taskbar'daki gibi uygulamanın kendi tepsi menüsü. Simgesi
      // henüz bilinmiyorsa (yeni açılmış) dock'un uygulama menüsü.
      t.oncontextmenu = async (e) => {
        e.preventDefault();
        ripple(e);
        const ok = await invoke("tray_menu", { exe: a.exe }).catch(() => false);
        if (!ok) invoke("app_menu", { item: { name: a.name, path: a.exe, args: null, aumid: "", windows: [], pinned: false } }).catch(() => {});
      };
      grid.append(t);
    });
    panel.append(grid);
  }
  const foot = el("div", "tray-foot");
  const set = el("button", "", "Bildirim alanı ayarları");
  set.onclick = () => { invoke("launch_app", { path: "ms-settings:taskbar" }); invoke("popover_close"); };
  const tm = el("button", "", "Görev Yöneticisi");
  tm.onclick = () => { invoke("launch_app", { path: "taskmgr.exe" }); invoke("popover_close"); };
  foot.append(set, tm);
  panel.append(foot);
}

// ---------------------------------------------------------------------------
// Takvim
// ---------------------------------------------------------------------------
let view = null; // gösterilen ay (ayın 1'i)
let selected = null;

function renderCalendar() {
  const today = new Date();
  view = new Date(today.getFullYear(), today.getMonth(), 1);
  selected = today;
  const cal = el("div", "cal");
  cal.append(el("div", "month"), el("aside", "side"));
  panel.append(cal);
  drawMonth();
  drawSide();
}

function drawMonth(dir) {
  const box = panel.querySelector(".month");
  const today = new Date();
  box.innerHTML = `
    <div class="m-head">
      <button class="m-title" title="Bugüne dön">${CAL.MONTHS[view.getMonth()]} ${view.getFullYear()}</button>
      <div class="m-nav">
        <button data-d="-1" aria-label="Önceki ay">${SVG.prev}</button>
        <button data-d="1" aria-label="Sonraki ay">${SVG.next}</button>
      </div>
    </div>
    <div class="dow"><span></span>${CAL.WEEKDAYS.map((d) => `<span>${d}</span>`).join("")}</div>
    <div class="grid-wrap"><div class="grid"></div></div>`;
  const grid = box.querySelector(".grid");
  if (dir) grid.classList.add(dir > 0 ? "slide-next" : "slide-prev");

  // Pazartesi başlangıçlı 6 hafta
  const first = new Date(view);
  const offset = (first.getDay() + 6) % 7;
  const start = new Date(first.getFullYear(), first.getMonth(), 1 - offset);
  for (let i = 0; i < 42; i++) {
    const d = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
    if (i % 7 === 0) grid.append(el("span", "wk", CAL.isoWeek(d)));
    const b = el("button", "day", d.getDate());
    const h = CAL.holiday(d);
    if (d.getMonth() !== view.getMonth()) b.classList.add("other");
    if (d.getDay() === 0 || d.getDay() === 6) b.classList.add("weekend");
    if (h) b.classList.add("holiday");
    if (CAL.sameDay(d, today)) b.classList.add("today");
    if (selected && CAL.sameDay(d, selected)) b.classList.add("selected");
    b.title = h ? `${h.name}${h.half ? " (yarım gün)" : ""}` : "";
    b.setAttribute("aria-label", `${d.getDate()} ${CAL.MONTHS[d.getMonth()]}${h ? ", " + h.name : ""}`);
    b.onclick = () => {
      selected = d;
      if (d.getMonth() !== view.getMonth()) {
        const step = d < view ? -1 : 1;
        view = new Date(d.getFullYear(), d.getMonth(), 1);
        drawMonth(step);
      } else {
        grid.querySelectorAll(".selected").forEach((x) => x.classList.remove("selected"));
        b.classList.add("selected");
      }
      drawSide();
    };
    grid.append(b);
  }

  box.querySelectorAll(".m-nav button").forEach((b) => {
    b.onclick = () => {
      const step = Number(b.dataset.d);
      view = new Date(view.getFullYear(), view.getMonth() + step, 1);
      drawMonth(step);
    };
  });
  box.querySelector(".m-title").onclick = goToday;
}

function goToday() {
  const t = new Date();
  const step = t < view ? -1 : t.getMonth() === view.getMonth() && t.getFullYear() === view.getFullYear() ? 0 : 1;
  view = new Date(t.getFullYear(), t.getMonth(), 1);
  selected = t;
  drawMonth(step || undefined);
  drawSide();
}

function drawSide() {
  const side = panel.querySelector(".side");
  const d = selected;
  const today = new Date();
  const h = CAL.holiday(d);
  const diff = CAL.daysBetween(today, d);
  const rel = diff === 0 ? "Bugün" : diff === 1 ? "Yarın" : diff === -1 ? "Dün" : diff > 0 ? `${diff} gün sonra` : `${-diff} gün önce`;
  const up = CAL.upcoming(today, 3);
  side.innerHTML = `
    <div class="big"><b>${d.getDate()}</b><span><strong>${CAL.DAYNAMES[d.getDay()]}</strong>${CAL.MONTHS[d.getMonth()]} ${d.getFullYear()}</span></div>
    <div class="chips"><span class="chip">${rel}</span><span class="chip">${CAL.isoWeek(d)}. hafta</span><span class="chip">Yılın ${CAL.dayOfYear(d)}. günü</span></div>
    ${h ? `<div class="hol-card">${esc(h.name)}<small>${h.half ? "Resmî tatil · yarım gün" : "Resmî tatil"}</small></div>` : ""}
    <h4>YAKLAŞAN TATİLLER</h4>
    <div class="up">${up
      .map(
        (u) => `<div><span class="d"><b>${u.date.getDate()}</b>${CAL.MONTHS[u.date.getMonth()].slice(0, 3)}</span>
        <span class="n">${esc(u.name)}<small>${u.days} gün sonra · ${CAL.DAYNAMES[u.date.getDay()]}</small></span></div>`,
      )
      .join("")}</div>
    <div class="side-foot">
      <button class="primary" data-a="today">Bugün</button>
      <button data-a="gcal" title="Google Takvim'i tarayıcıda aç">Google Takvim</button>
    </div>`;
  side.querySelector('[data-a="today"]').onclick = goToday;
  side.querySelector('[data-a="gcal"]').onclick = () => {
    const p = (n) => String(n).padStart(2, "0");
    invoke("launch_app", { path: `https://calendar.google.com/calendar/r/day/${d.getFullYear()}/${p(d.getMonth() + 1)}/${p(d.getDate())}` });
    invoke("popover_close");
  };
}

// ---------------------------------------------------------------------------
// Ortak: grafik ipucu (fareyle üzerine gelince değer)
// ---------------------------------------------------------------------------
let tipEl = null;
function bindTips(root) {
  root.querySelectorAll("[data-tip]").forEach((n) => {
    n.addEventListener("mouseenter", () => {
      if (!tipEl) { tipEl = el("div", "tip"); document.body.append(tipEl); }
      tipEl.innerHTML = n.dataset.tip;
      const r = n.getBoundingClientRect();
      tipEl.style.display = "block";
      const tw = tipEl.offsetWidth, th = tipEl.offsetHeight;
      const pr = panel.getBoundingClientRect();
      const x = Math.max(pr.left + 6, Math.min(pr.right - tw - 6, r.left + r.width / 2 - tw / 2));
      tipEl.style.left = `${x}px`;
      tipEl.style.top = `${Math.max(pr.top + 4, r.top - th - 6)}px`;
    });
    n.addEventListener("mouseleave", () => { if (tipEl) tipEl.style.display = "none"; });
  });
}
const hideTip = () => { if (tipEl) tipEl.style.display = "none"; };

let liveTimer = null;
function stopLive() { clearInterval(liveTimer); liveTimer = null; hideTip(); }

// ---------------------------------------------------------------------------
// Kullanım paneli: ekran süresi, uygulamalar, kaynaklar, sistem bilgisi
// ---------------------------------------------------------------------------
const RES = [
  { k: "cpu", name: "İşlemci", cls: "m-cpu" },
  { k: "ram", name: "Bellek", cls: "m-ram" },
  { k: "gpu", name: "Ekran kartı", cls: "m-gpu" },
  { k: "disk", name: "Disk etkinliği", cls: "m-disk" },
];
const dur = (secs) => {
  const h = Math.floor(secs / 3600), m = Math.floor((secs % 3600) / 60);
  return h ? `${h} sa ${m} dk` : `${m} dk`;
};
const durShort = (secs) => {
  const h = Math.floor(secs / 3600), m = Math.floor((secs % 3600) / 60);
  return h ? `${h}s${m ? " " + m + "dk" : ""}` : `${m}dk`;
};
const DAY_SHORT = ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"];
const pctTxt = (v) => (v < 0 ? "–" : `%${Math.round(v)}`);

async function renderUsage() {
  const d = await invoke("usage_data").catch(() => null);
  if (!d) { panel.append(el("div", "tray-empty", "Kullanım verisi alınamadı.")); return; }
  const st = d.screen;
  const now = new Date();

  // --- Ekran süresi: kahraman sayı + karşılaştırma
  const past = (st.history || []).filter((h) => h.seconds > 0);
  const avg = past.length ? past.reduce((a, h) => a + h.seconds, 0) / past.length : 0;
  const delta = avg > 0 ? `Son 7 günün ortalaması <b>${dur(avg)}</b>` : "";
  const peakH = st.hours.reduce((best, v, i) => (v > st.hours[best] ? i : best), 0);

  // Saatlik sütunlar (00–23)
  const maxH = Math.max(600, ...st.hours);
  const hourCols = st.hours.map((v, i) => {
    const cur = i === now.getHours();
    const h = Math.round((v / maxH) * 100);
    return `<span class="col${cur ? " cur" : ""}${i > now.getHours() ? " future" : ""}" data-tip="<b>${String(i).padStart(2, "0")}:00–${String(i + 1).padStart(2, "0")}:00</b><br>${dur(v)}"><i style="height:${Math.max(v ? 4 : 0, h)}%"></i></span>`;
  }).join("");

  // Son 7 gün (6 geçmiş + bugün)
  const days = [];
  for (let i = 6; i >= 0; i--) {
    const dt = new Date(now.getFullYear(), now.getMonth(), now.getDate() - i);
    const key = `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, "0")}-${String(dt.getDate()).padStart(2, "0")}`;
    const secs = i === 0 ? st.seconds : (st.history || []).find((h) => h.date === key)?.seconds || 0;
    days.push({ dt, secs, today: i === 0 });
  }
  const maxD = Math.max(3600, ...days.map((x) => x.secs));
  const weekCols = days.map((x) => `
    <span class="col wk${x.today ? " cur" : ""}" data-tip="<b>${x.today ? "Bugün" : CAL.DAYNAMES[x.dt.getDay()]}</b>, ${x.dt.getDate()} ${CAL.MONTHS[x.dt.getMonth()]}<br>${dur(x.secs)}">
      <em>${x.secs ? durShort(x.secs) : ""}</em><i style="height:${Math.max(x.secs ? 4 : 0, Math.round((x.secs / maxD) * 100))}%"></i><small>${x.today ? "Bugün" : DAY_SHORT[x.dt.getDay()]}</small>
    </span>`).join("");

  // --- Uygulamalar: ilk 5 + diğer, halka grafik
  const apps = Object.values(st.apps || {}).sort((a, b) => b.seconds - a.seconds);
  const total = apps.reduce((a, x) => a + x.seconds, 0);
  const top = apps.slice(0, 5);
  const rest = apps.slice(5).reduce((a, x) => a + x.seconds, 0);
  const segs = top.map((a, i) => ({ name: a.name, path: a.path, secs: a.seconds, color: `var(--s${i + 1})` }));
  if (rest > 0) segs.push({ name: `Diğer (${apps.length - 5})`, path: "", secs: rest, color: "var(--other)" });
  const R = 46, C = 2 * Math.PI * R, GAP = segs.length > 1 ? 2.5 : 0;
  let off = 0;
  const arcs = segs.map((sg) => {
    const len = total ? (sg.secs / total) * C : 0;
    const a = `<circle cx="60" cy="60" r="${R}" stroke="${sg.color}" stroke-dasharray="${Math.max(0, len - GAP)} ${C}" stroke-dashoffset="${-off}" data-tip="<b>${esc(sg.name)}</b><br>${dur(sg.secs)} · %${Math.round((sg.secs / total) * 100)}"/>`;
    off += len;
    return a;
  }).join("");
  const legend = segs.map((sg, i) => `
    <div class="lg" data-tip="<b>${esc(sg.name)}</b><br>${dur(sg.secs)}">
      <span class="sw" style="background:${sg.color}"></span>
      <span class="lg-ic" data-i="${i}"></span>
      <span class="lg-n">${esc(sg.name)}</span>
      <span class="lg-v">${durShort(sg.secs)}</span>
    </div>`).join("");

  const wrap = el("div", "use");
  wrap.innerHTML = `
    <section class="use-col">
      <h4>EKRAN SÜRESİ · BUGÜN</h4>
      <div class="hero"><b>${dur(st.seconds)}</b>${delta ? `<span>${delta}</span>` : ""}</div>
      <div class="chips">
        ${st.first ? `<span class="chip">İlk etkinlik ${st.first}</span>` : ""}
        ${st.seconds ? `<span class="chip">En yoğun ${String(peakH).padStart(2, "0")}:00</span>` : ""}
        <span class="chip">${st.switches || 0} uygulama geçişi</span>
      </div>
      <div class="chart-h">Saatlik</div>
      <div class="cols hours">${hourCols}</div>
      <div class="axis"><span>00</span><span>06</span><span>12</span><span>18</span><span>23</span></div>
      <div class="chart-h">Son 7 gün</div>
      <div class="cols week">${weekCols}</div>
    </section>
    <section class="use-col">
      <h4>UYGULAMALAR</h4>
      ${segs.length ? `<div class="apps-row">
        <div class="donut"><svg viewBox="0 0 120 120" width="120" height="120">${arcs}</svg>
          <span class="donut-c"><b>${apps.length}</b><small>uygulama</small></span></div>
        <div class="legend">${legend}</div>
      </div>` : `<div class="empty-note">Uygulama kullanımı toplanıyor; birkaç dakika sonra burada görünecek.</div>`}
      <h4 class="res-h">KAYNAKLAR <span>son 2 dakika</span></h4>
      <div class="res-grid"></div>
    </section>`;
  panel.append(wrap);

  // Uygulama simgeleri
  segs.forEach((sg, i) => {
    if (!sg.path) return;
    invoke("app_icon", { path: sg.path, aumid: null, hwnd: null }).then((url) => {
      const slot = wrap.querySelector(`.lg-ic[data-i="${i}"]`);
      if (url && slot) slot.append(Object.assign(new Image(), { src: url, alt: "" }));
    }).catch(() => {});
  });

  const sys = el("div", "sysbar");
  panel.append(sys);
  drawResources(wrap.querySelector(".res-grid"), d);
  drawSysbar(sys, d);
  bindTips(panel);

  stopLive();
  liveTimer = setInterval(async () => {
    if (spec?.kind !== "usage") return stopLive();
    const n = await invoke("usage_data").catch(() => null);
    if (!n) return;
    drawResources(wrap.querySelector(".res-grid"), n);
    drawSysbar(sys, n);
  }, 1000);
}

function areaPath(vals, w, h) {
  if (vals.length < 2) return { line: "", area: "" };
  const step = w / 119; // 120 örnek = 2 dk; az veri varsa sağa yaslı
  const x0 = w - (vals.length - 1) * step;
  const pts = vals.map((v, i) => [x0 + i * step, h - (Math.max(0, Math.min(100, v)) / 100) * (h - 2) - 1]);
  const line = pts.map((p, i) => `${i ? "L" : "M"}${p[0].toFixed(1)},${p[1].toFixed(1)}`).join("");
  return { line, area: `${line}L${w},${h}L${x0.toFixed(1)},${h}Z` };
}

function drawResources(grid, d) {
  const cur = d.now;
  const items = RES.filter((r) => cur[r.k] >= 0);
  if (grid.childElementCount !== items.length) {
    grid.innerHTML = items.map((r) => `
      <div class="res ${r.cls}">
        <div class="res-top">
          <span class="ring-wrap"><svg class="ring" viewBox="0 0 40 40" width="38" height="38"><circle cx="20" cy="20" r="15" class="ring-track"/><circle cx="20" cy="20" r="15" class="ring-val"/></svg></span>
          <span class="res-t"><small>${r.name}</small><b></b></span>
          <span class="res-d"></span>
        </div>
        <svg class="area" viewBox="0 0 150 34" preserveAspectRatio="none"><line x1="0" x2="150" y1="17" y2="17" class="grid"/><path class="ar"/><path class="ln"/></svg>
      </div>`).join("");
  }
  const C = 2 * Math.PI * 15;
  items.forEach((r, i) => {
    const box = grid.children[i];
    const v = Math.max(0, cur[r.k]);
    const ringV = box.querySelector(".ring-val");
    ringV.setAttribute("stroke-dasharray", C);
    ringV.setAttribute("stroke-dashoffset", C * (1 - v / 100));
    box.querySelector(".res-t b").textContent = pctTxt(cur[r.k]);
    const vals = d.history.map((h) => Math.max(0, h[r.k]));
    const avg = vals.length ? vals.reduce((a, x) => a + x, 0) / vals.length : 0;
    const peak = vals.length ? Math.max(...vals) : 0;
    box.querySelector(".res-d").innerHTML = r.k === "ram"
      ? `${cur.ram_used_gb.toFixed(1)} / ${cur.ram_total_gb.toFixed(1)} GB`
      : `ort. %${Math.round(avg)} · en çok %${Math.round(peak)}`;
    const { line, area } = areaPath(vals, 150, 34);
    box.querySelector(".ln").setAttribute("d", line);
    box.querySelector(".ar").setAttribute("d", area);
  });
}

function drawSysbar(sys, d) {
  const i = d.info;
  const up = i.uptime_sec;
  const upTxt = up >= 86400 ? `${Math.floor(up / 86400)} gün ${Math.floor((up % 86400) / 3600)} sa` : dur(up);
  const used = i.disk_total_gb - i.disk_free_gb;
  const diskPct = i.disk_total_gb ? Math.round((used / i.disk_total_gb) * 100) : 0;
  const html = `
    <div class="si" title="${esc(i.os)}"><small>Sistem</small><b>${esc((i.os || "Windows").split(" · ")[0])}</b></div>
    <div class="si" title="${esc(i.cpu_name)}"><small>İşlemci</small><b>${esc((i.cpu_name || "–").replace(/\(R\)|\(TM\)|CPU|Processor|@.*$| w\/.*$|with .*$/gi, "").replace(/\s+/g, " ").trim())}${i.cores ? ` · ${i.cores} iş parçacığı` : ""}</b></div>
    <div class="si"><small>Açık kalma</small><b>${upTxt}</b></div>
    <div class="si disk"><small>C: diski · ${i.disk_free_gb.toFixed(0)} GB boş</small><span class="dbar"><i style="width:${diskPct}%"></i></span></div>
    ${i.battery >= 0 ? `<div class="si"><small>Pil</small><b>%${i.battery}${i.charging ? " · şarjda" : ""}</b></div>` : ""}
    <div class="si"><small>İşlem</small><b>${i.procs}</b></div>
    <button class="tm">Görev Yöneticisi</button>`;
  if (sys.dataset.sig === html) return;
  sys.dataset.sig = html;
  sys.innerHTML = html;
  sys.querySelector(".tm").onclick = () => { invoke("launch_app", { path: "taskmgr.exe" }); invoke("popover_close"); };
}

// ---------------------------------------------------------------------------
// Haber ve piyasa paneli
// ---------------------------------------------------------------------------
const newsCache = new Map(); // url -> { at, items }
let newsCat = "top";

async function loadFeed(url) {
  const c = newsCache.get(url);
  if (c && Date.now() - c.at < 10 * 60_000) return c.items;
  const items = parseFeed(await invoke("fetch_text", { url }), new URL(url).hostname);
  newsCache.set(url, { at: Date.now(), items });
  return items;
}

async function renderNews(p) {
  const cats = [...NEWS_CATEGORIES];
  const own = (p?.feeds || []).filter((u) => !cats.some((c) => c.url === u));
  if (own.length) cats.push({ id: "mine", name: "Kaynaklarım", urls: own });
  if (!cats.some((c) => c.id === newsCat)) newsCat = "top";

  const box = el("div", "news");
  box.innerHTML = `
    <aside class="mk">
      <h4>PİYASALAR <span class="upd"></span></h4>
      <div class="mk-list"><div class="mk-load">Yükleniyor…</div></div>
    </aside>
    <section class="nw">
      <div class="tabs" role="tablist">${cats.map((c) => `<button role="tab" data-c="${c.id}" class="${c.id === newsCat ? "on" : ""}">${c.name}</button>`).join("")}</div>
      <div class="nw-list"></div>
      <div class="nw-foot"><span class="nw-src"></span><button class="nw-refresh">Yenile</button></div>
    </section>`;
  panel.append(box);

  const list = box.querySelector(".nw-list");
  async function showCat(id, force) {
    newsCat = id;
    box.querySelectorAll(".tabs button").forEach((b) => b.classList.toggle("on", b.dataset.c === id));
    const cat = cats.find((c) => c.id === id);
    const urls = cat.urls || [cat.url];
    if (force) urls.forEach((u) => newsCache.delete(u));
    list.innerHTML = `<div class="nw-empty">Haberler yükleniyor…</div>`;
    const all = [];
    for (const u of urls) {
      try { all.push(...(await loadFeed(u))); } catch (e) { console.warn("haber alınamadı", u, e); }
    }
    if (newsCat !== id) return;
    const seen = new Set();
    const items = all.filter((n) => !seen.has(n.title) && seen.add(n.title))
      .sort((a, b) => (b.date || 0) - (a.date || 0)).slice(0, 30);
    if (!items.length) { list.innerHTML = `<div class="nw-empty">Bu kategoride haber alınamadı.</div>`; return; }
    list.replaceChildren(...items.map((n, i) => {
      const b = el("button", "nw-item" + (i === 0 ? " lead" : ""));
      const ago = n.date && !isNaN(n.date) ? timeAgo(n.date) : "";
      b.innerHTML = `<span class="nw-meta"><b></b>${ago ? ` · ${ago}` : ""}</span><span class="nw-title"></span>`;
      b.querySelector(".nw-meta b").textContent = n.source;
      b.querySelector(".nw-title").textContent = n.title;
      b.title = "Haberi tarayıcıda aç";
      if (p?.focus && n.link === p.focus) b.classList.add("focus");
      b.onclick = () => { if (n.link) invoke("launch_app", { path: n.link }); invoke("popover_close"); };
      return b;
    }));
    list.scrollTop = 0;
    box.querySelector(".nw-src").textContent = `${items.length} haber · ${id === "mine" ? "kendi kaynakların" : "Google Haberler"}`;
  }
  box.querySelectorAll(".tabs button").forEach((b) => (b.onclick = () => showCat(b.dataset.c)));
  box.querySelector(".nw-refresh").onclick = () => { showCat(newsCat, true); loadMarkets(true); };

  const mk = box.querySelector(".mk-list");
  let mkData = null;
  async function loadMarkets(force) {
    try {
      if (force || !mkData || Date.now() - mkData.at > 5 * 60_000) {
        mkData = { at: Date.now(), data: JSON.parse(await invoke("fetch_text", { url: "https://finans.truncgil.com/today.json" })) };
      }
    } catch { mk.innerHTML = `<div class="mk-load">Piyasa verisi alınamadı.</div>`; return; }
    const data = mkData.data;
    const mine = new Set(p?.markets || []);
    const upd = data.Update_Date ? new Date(data.Update_Date.replace(" ", "T")) : null;
    box.querySelector(".upd").textContent = upd && !isNaN(upd) ? `· ${upd.toLocaleTimeString("tr-TR", { hour: "2-digit", minute: "2-digit" })}` : "";
    mk.innerHTML = MARKET_GROUPS.map(([g, keys]) => {
      const rows = keys.filter((k) => data[k] && marketInfo(k)).map((k) => {
        const m = marketInfo(k);
        const sell = trNum(data[k]["Satış"]), buy = trNum(data[k]["Alış"]), chg = trNum(data[k]["Değişim"]);
        const dir = chg > 0 ? "up" : chg < 0 ? "down" : "flat";
        const unit = k === "ons" ? "$" : "₺";
        return `<div class="mk-row${mine.has(k) ? " mine" : ""}" title="${m.name}: alış ${fmt2(buy)} ${unit} · satış ${fmt2(sell)} ${unit}${mine.has(k) ? " · dock'ta gösteriliyor" : ""}">
          <span class="mk-g" style="background:${m.bg}">${m.glyph}</span>
          <span class="mk-n"><b>${m.name}</b><small>Alış ${fmt2(buy)}</small></span>
          <span class="mk-v"><b>${fmt2(sell)} ${unit}</b><span class="chg ${dir}">${dir === "up" ? "▲" : dir === "down" ? "▼" : "•"} %${fmt2(Math.abs(chg))}</span></span>
        </div>`;
      }).join("");
      return rows ? `<div class="mk-g-h">${g}</div>${rows}` : "";
    }).join("");
  }
  loadMarkets();
  showCat(newsCat);
}

// ---------------------------------------------------------------------------
// Hava durumu paneli
// ---------------------------------------------------------------------------
const hhmm = (iso) => (iso || "").slice(11, 16);
async function renderWeather() {
  const cfg = pcfg || (await invoke("get_config").catch(() => ({})));
  let w;
  try { w = await Weather.get(cfg); } catch (e) {
    panel.append(el("div", "tray-empty", "Hava durumu alınamadı. İnternet bağlantını kontrol et."));
    return;
  }
  const cur = w.current, d = w.daily, h = w.hourly;
  const now = new Date();
  // Şu anki saatten başlayan 24 saat
  const pad = (n) => String(n).padStart(2, "0");
  const key = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}T${pad(now.getHours())}:00`;
  let i0 = h.time.indexOf(key);
  if (i0 < 0) i0 = 0;
  const hrs = Array.from({ length: 25 }, (_, k) => i0 + k).filter((i) => i < h.time.length);
  const temps = hrs.map((i) => h.temperature_2m[i]);
  const tMin = Math.min(...temps), tMax = Math.max(...temps);
  const W = 640, H = 70, padY = 10;
  const xs = (k) => (k / (hrs.length - 1)) * W;
  const ys = (t) => padY + (1 - (t - tMin) / Math.max(1, tMax - tMin)) * (H - padY * 2);
  const line = hrs.map((_, k) => `${k ? "L" : "M"}${xs(k).toFixed(1)},${ys(temps[k]).toFixed(1)}`).join("");
  const area = `${line}L${W},${H}L0,${H}Z`;
  const rain = hrs.map((i, k) => {
    const p = h.precipitation_probability[i] || 0;
    return p ? `<rect x="${(xs(k) - 6).toFixed(1)}" y="${H - (p / 100) * 26}" width="12" height="${(p / 100) * 26}" rx="2" class="rain"/>` : "";
  }).join("");
  const hits = hrs.map((i, k) => `<rect x="${(xs(k) - W / hrs.length / 2).toFixed(1)}" y="0" width="${(W / hrs.length).toFixed(1)}" height="${H}" fill="transparent"
      data-tip="<b>${hhmm(h.time[i])}</b> · ${esc(wxText(h.weather_code[i]))}<br>${Math.round(h.temperature_2m[i])}° · yağış %${h.precipitation_probability[i] ?? 0}"/>`).join("");
  const ticks = hrs.filter((_, k) => k % 3 === 0).map((i) => {
    const k = hrs.indexOf(i);
    return `<div class="hr" style="left:${(xs(k) / W) * 100}%">
      <small>${k === 0 ? "Şimdi" : hhmm(h.time[i])}</small>${wxIcon(h.weather_code[i], h.is_day[i] === 1, 22)}<b>${Math.round(h.temperature_2m[i])}°</b></div>`;
  }).join("");

  // Günlük: bugün + 5 gün; aralık çubuğu haftanın en düşük/en yüksek değerine göre
  const days = d.time.slice(0, 6).map((t, i) => ({ t, i }));
  const wMin = Math.min(...d.temperature_2m_min.slice(0, 6)), wMax = Math.max(...d.temperature_2m_max.slice(0, 6));
  const pos = (t) => ((t - wMin) / Math.max(1, wMax - wMin)) * 100;
  const rows = days.map(({ t, i }) => {
    const dt = new Date(t + "T12:00");
    const name = i === 0 ? "Bugün" : i === 1 ? "Yarın" : CAL.DAYNAMES[dt.getDay()];
    const lo = d.temperature_2m_min[i], hi = d.temperature_2m_max[i];
    const p = d.precipitation_probability_max[i] ?? 0;
    const dot = i === 0 ? `<i class="now" style="left:${pos(cur.temperature_2m)}%"></i>` : "";
    return `<div class="day-row" data-tip="<b>${name}</b> · ${esc(wxText(d.weather_code[i]))}<br>${Math.round(lo)}° / ${Math.round(hi)}° · yağış %${p}${d.uv_index_max ? ` · UV ${Math.round(d.uv_index_max[i])}` : ""}">
      <span class="dn">${name}<small>${dt.getDate()} ${CAL.MONTHS[dt.getMonth()].slice(0, 3)}</small></span>
      ${wxIcon(d.weather_code[i], true, 24)}
      <span class="pp${p >= 40 ? " hi" : ""}">${p >= 10 ? `%${p}` : ""}</span>
      <span class="lo">${Math.round(lo)}°</span>
      <span class="range"><span class="fill" style="left:${pos(lo)}%;right:${100 - pos(hi)}%"></span>${dot}</span>
      <span class="hi">${Math.round(hi)}°</span>
    </div>`;
  }).join("");

  const box = el("div", "wxp");
  box.innerHTML = `
    <div class="wx-now">
      <div class="wx-big">${wxIcon(cur.weather_code, cur.is_day === 1, 58)}
        <div><b>${Math.round(cur.temperature_2m)}°</b><span>${esc(wxText(cur.weather_code))}</span></div></div>
      <div class="wx-facts">
        <span><small>Hissedilen</small>${Math.round(cur.apparent_temperature)}°</span>
        <span><small>Nem</small>%${cur.relative_humidity_2m}</span>
        <span><small>Rüzgâr</small>${Math.round(cur.wind_speed_10m)} km/s</span>
        <span><small>Gün doğumu</small>${hhmm(d.sunrise[0])}</span>
        <span><small>Gün batımı</small>${hhmm(d.sunset[0])}</span>
      </div>
      <div class="wx-place"><b>${esc(w.city || "")}</b><small>En yüksek ${Math.round(d.temperature_2m_max[0])}° · en düşük ${Math.round(d.temperature_2m_min[0])}°</small></div>
    </div>
    <h4>SONRAKİ 24 SAAT</h4>
    <div class="hours">
      <div class="hr-row">${ticks}</div>
      <svg class="tchart" viewBox="0 0 ${W} ${H}" preserveAspectRatio="none">${rain}<path class="ar" d="${area}"/><path class="ln" d="${line}"/>${hits}</svg>
      <div class="legend-s"><span><i class="ln"></i>Sıcaklık</span><span><i class="rn"></i>Yağış olasılığı</span></div>
    </div>
    <h4>6 GÜNLÜK TAHMİN</h4>
    <div class="days">${rows}</div>
    <div class="wx-foot"><span>Kaynak: Open-Meteo · ${now.toLocaleTimeString("tr-TR", { hour: "2-digit", minute: "2-digit" })}</span><button class="loc">Konum ayarları</button></div>`;
  panel.append(box);
  box.querySelector(".loc").onclick = () => { invoke("open_settings"); invoke("popover_close"); };
  bindTips(panel);
}

// ---------------------------------------------------------------------------
// Pano geçmişi paneli
// ---------------------------------------------------------------------------
const CLIP_SVG = {
  pin: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 4h6l-1 6 4 4H6l4-4z"/><path d="M12 14v7"/></svg>',
  del: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/></svg>',
  ok: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12l5 5 9-10"/></svg>',
};
const clipIcons = new Map();
let clipUnlisten = null;

async function renderClipboard() {
  const box = el("div", "clp");
  box.innerHTML = `
    <div class="clp-head"><strong>Pano geçmişi</strong><span class="clp-n"></span><button class="clp-clear">Temizle</button></div>
    <div class="clp-list"></div>
    <div class="clp-foot">Tıkla: tekrar kopyalanır. Geçmiş yalnızca bellekte tutulur; şifre yöneticilerinden kopyalananlar kaydedilmez.</div>`;
  panel.append(box);
  const list = box.querySelector(".clp-list");
  box.querySelector(".clp-clear").onclick = () => invoke("clip_clear").catch(() => {});

  async function draw() {
    const items = (await invoke("clip_list").catch(() => [])) || [];
    box.querySelector(".clp-n").textContent = items.length ? `${items.length} öğe` : "";
    box.querySelector(".clp-clear").style.visibility = items.some((x) => !x.pinned) ? "visible" : "hidden";
    if (!items.length) {
      list.innerHTML = `<div class="clp-empty">Henüz bir şey kopyalamadın.<br>Kopyaladığın metin ve görseller burada birikir.</div>`;
      return;
    }
    list.replaceChildren(...items.map((it) => {
      const b = el("button", "clp-item" + (it.pinned ? " pinned" : "") + (it.kind === "image" ? " img" : ""));
      const ago = timeAgo(new Date(it.time)) || "şimdi";
      const meta = it.kind === "image" ? `${it.width}×${it.height}` : `${it.text.length.toLocaleString("tr-TR")} karakter`;
      b.innerHTML = `
        ${it.kind === "image" ? `<img class="clp-img" src="${it.preview}" alt="">` : `<span class="clp-text"></span>`}
        <span class="clp-meta"><span class="clp-src"></span><span>${esc(it.source || "")}</span><span>· ${ago} · ${meta}</span></span>
        <span class="clp-acts">
          <span class="clp-act pin${it.pinned ? " on" : ""}" title="${it.pinned ? "Sabitlemeyi kaldır" : "Sabitle (temizlenmez)"}">${CLIP_SVG.pin}</span>
          <span class="clp-act del" title="Sil">${CLIP_SVG.del}</span>
        </span>
        <span class="clp-done">${CLIP_SVG.ok} Kopyalandı</span>`;
      if (it.kind === "text") b.querySelector(".clp-text").textContent = it.text.length > 600 ? it.text.slice(0, 600) + "…" : it.text;
      if (it.source_path) {
        const slot = b.querySelector(".clp-src");
        const put = (url) => { if (url) slot.innerHTML = `<img src="${url}" alt="">`; };
        if (clipIcons.has(it.source_path)) put(clipIcons.get(it.source_path));
        else invoke("app_icon", { path: it.source_path, aumid: null, hwnd: null }).then((u) => { clipIcons.set(it.source_path, u); put(u); }).catch(() => {});
      }
      b.onclick = async (e) => {
        if (e.target.closest(".pin")) return invoke("clip_pin", { id: it.id, pinned: !it.pinned });
        if (e.target.closest(".del")) return invoke("clip_remove", { id: it.id });
        const ok = await invoke("clip_copy", { id: it.id }).catch(() => false);
        if (ok !== false) {
          b.classList.add("copied");
          setTimeout(() => invoke("popover_close"), 650);
        }
      };
      return b;
    }));
  }
  await draw();
  clipUnlisten?.();
  clipUnlisten = await listen("clip-changed", () => { if (spec?.kind === "clipboard") draw(); });
}

// ---------------------------------------------------------------------------
// Claude kullanımı paneli
// ---------------------------------------------------------------------------
function untilTxt(iso) {
  const t = new Date(iso);
  if (!iso || isNaN(t)) return "";
  const m = Math.round((t - Date.now()) / 60000);
  if (m <= 0) return "şimdi sıfırlanıyor";
  const d = Math.floor(m / 1440), h = Math.floor((m % 1440) / 60), mm = m % 60;
  const rel = d ? `${d} gün ${h} sa` : h ? `${h} sa ${mm} dk` : `${mm} dk`;
  const day = t.toDateString() === new Date().toDateString() ? "bugün" : CAL.DAYNAMES[t.getDay()];
  return `${rel} sonra sıfırlanır · ${day} ${t.toLocaleTimeString("tr-TR", { hour: "2-digit", minute: "2-digit" })}`;
}
let claudeUnlisten = null;
async function renderClaude() {
  const box = el("div", "cld");
  panel.append(box);
  const draw = (u) => {
    if (!u) return;
    const tone = (p) => (p >= 85 ? "hi" : p >= 60 ? "mid" : "lo");
    const state = {
      ok: "",
      stale: "Son bilinen değerler gösteriliyor.",
      expired: "Claude Code oturum anahtarının süresi dolmuş. Claude Code'u bir kez açınca yenilenir.",
      no_claude_code: "Bu bilgisayarda Claude Code oturumu bulunamadı. Claude Code'u kurup bir kez giriş yaparsan limitlerin burada görünür.",
      error: "Kullanım okunamadı.",
      loading: "Yükleniyor…",
    }[u.state] ?? "";
    const upd = u.updated ? `Son güncelleme ${timeAgo(new Date(u.updated)) || "şimdi"}` : "";
    box.innerHTML = `
      <div class="cld-head"><span class="cld-tile">${'<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6L5.6 18.4"/></svg>'}</span>
        <div><strong>Claude kullanımı</strong><span>${u.plan ? esc(u.plan) + " planı" : "Plan bilinmiyor"}</span></div></div>
      ${(u.limits || []).map((l) => `
        <div class="cld-row ${tone(l.percent)}">
          <div class="cld-top"><span>${esc(l.label === "Oturum" ? "Oturum (5 saatlik)" : l.label === "Haftalık" ? "Haftalık (tüm modeller)" : "Haftalık · " + l.label)}</span><b>%${Math.round(l.percent)}</b></div>
          <span class="cld-bar"><i style="width:${Math.min(100, l.percent)}%"></i></span>
          <small>${untilTxt(l.resets_at)}</small>
        </div>`).join("")}
      ${state || u.message ? `<div class="cld-note">${esc(state)}${u.message && u.state !== "ok" ? `<br><small>${esc(u.message)}</small>` : ""}</div>` : ""}
      <div class="cld-foot"><span>${upd}</span><button class="cld-refresh">Yenile</button></div>
      <div class="cld-src">Veriler Claude Code'un /usage ekranıyla aynı kaynaktan okunur (resmî olmayan uç nokta). Haftalık limit claude.ai ile ortaktır.</div>`;
    box.querySelector(".cld-refresh").onclick = () => invoke("claude_refresh").catch(() => {});
    relayout();
  };
  draw(await invoke("claude_usage").catch(() => null));
  claudeUnlisten?.();
  claudeUnlisten = await listen("claude-usage", (e) => { if (spec?.kind === "claude") draw(e.payload); });
}

// ---------------------------------------------------------------------------
// Diğer medya oynatıcıları (dock'ta görünen hariç)
// ---------------------------------------------------------------------------
const MSVG = {
  prev: '<svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"><path d="M19 5v14l-10-7zM5 5h2v14H5z"/></svg>',
  next: '<svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"><path d="M5 5v14l10-7zM17 5h2v14h-2z"/></svg>',
  play: '<svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"><path d="M7 4l13 8-13 8z"/></svg>',
  pause: '<svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"><path d="M6 4h4v16H6zM14 4h4v16h-4z"/></svg>',
  wave: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M5 10v4M9 7v10M13 9v6M17 6v12M21 10v4"/></svg>',
};
const mediaArt = new Map();
async function renderMedia(p) {
  const box = el("div", "mdl");
  panel.append(box);
  const exclude = p?.exclude;
  const cards = new Map();

  function card(m) {
    const c = el("div", "mdc");
    c.innerHTML = `
      <span class="mdc-art">${MSVG.wave}</span>
      <span class="mdc-meta"><b></b><small></small></span>
      <span class="mdc-ctl">
        <button data-a="prev" aria-label="Önceki">${MSVG.prev}</button>
        <button data-a="toggle" class="main" aria-label="Oynat/Duraklat"></button>
        <button data-a="next" aria-label="Sonraki">${MSVG.next}</button>
      </span>
      <span class="mdc-prog"><i></i></span>`;
    c.querySelectorAll("button").forEach((b) => {
      b.onclick = () => invoke("media_control_id", { id: c.dataset.id, action: b.dataset.a }).then(() => setTimeout(poll, 250)).catch(() => {});
    });
    return c;
  }
  function fill(c, m) {
    c.dataset.id = m.id;
    c.classList.toggle("playing", !!m.playing);
    c.querySelector(".mdc-meta b").textContent = m.title || "Bilinmeyen parça";
    c.querySelector(".mdc-meta small").textContent = [m.artist, m.source.replace(/\.exe$/i, "").split("!").pop()].filter(Boolean).join(" · ");
    c.querySelector(".main").innerHTML = m.playing ? MSVG.pause : MSVG.play;
    c.querySelector(".mdc-prog i").style.width = m.duration_ms > 0 ? `${(m.position_ms / m.duration_ms) * 100}%` : "0%";
    const k = `${m.id}\u0001${m.track_key}`;
    const art = c.querySelector(".mdc-art");
    const put = (url) => { if (url) { art.style.backgroundImage = `url("${url}")`; art.classList.add("img"); } };
    if (mediaArt.has(k)) put(mediaArt.get(k));
    else {
      mediaArt.set(k, null);
      invoke("media_thumbnail_id", { id: m.id, trackKey: m.track_key }).then((u) => { mediaArt.set(k, u); put(u); }).catch(() => {});
    }
  }
  async function poll() {
    if (spec?.kind !== "media") return stopLive();
    const list = ((await invoke("media_list").catch(() => [])) || []).filter((m) => m.active && m.id !== exclude);
    if (!list.length) { invoke("popover_close"); return; }
    // Çalanlar dock'a en yakın (altta)
    list.sort((a, b) => Number(a.playing) - Number(b.playing));
    const ids = list.map((m) => m.id).join("|");
    if (box.dataset.ids !== ids) {
      box.dataset.ids = ids;
      box.replaceChildren(...list.map((m) => {
        const c = cards.get(m.id) || card(m);
        cards.set(m.id, c);
        return c;
      }));
      relayout();
    }
    list.forEach((m) => fill(cards.get(m.id), m));
  }
  await poll();
  stopLive();
  liveTimer = setInterval(poll, 1000);
}

// ---------------------------------------------------------------------------
// Hızlı ayarlar (Windows'un Win+A paneli yerine)
// ---------------------------------------------------------------------------
const QI = {
  wifi: (n = 4) => `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">${[
    '<path d="M2 8.5a15 15 0 0 1 20 0"/>', '<path d="M5 12a10 10 0 0 1 14 0"/>', '<path d="M8.5 15.5a5 5 0 0 1 7 0"/>', '<circle cx="12" cy="19" r="1" fill="currentColor"/>',
  ].map((p, i) => (i >= 4 - n || i === 3 ? p : p.replace("<path", '<path opacity=".25"'))).join("")}</svg>`,
  bt: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M7 7l10 10-5 5V2l5 5L7 17"/></svg>',
  plane: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="M10.5 3.5c0-.8.7-1.5 1.5-1.5s1.5.7 1.5 1.5V9l7.5 4.5V16l-7.5-2.5V18l2 1.5V21L12 20l-3.5 1v-1.5l2-1.5v-4.5L3 16v-2.5L10.5 9z"/></svg>',
  leaf: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 19c0-8 5-13 15-14-1 10-6 15-14 15"/><path d="M5 19l7-7"/></svg>',
  moon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"/></svg>',
  hotspot: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="2"/><path d="M8 8a5.6 5.6 0 0 0 0 8M16 8a5.6 5.6 0 0 1 0 8M5 5a10 10 0 0 0 0 14M19 5a10 10 0 0 1 0 14"/></svg>',
  vol: (m, v) => `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 9h4l5-4v14l-5-4H4z"/>${m ? '<path d="M17 9l5 6M22 9l-5 6"/>' : (v > 0 ? '<path d="M16.5 9.5a3.5 3.5 0 0 1 0 5"/>' : "") + (v > 50 ? '<path d="M19 7a7 7 0 0 1 0 10"/>' : "")}</svg>`,
  sun: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>',
  chev: '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6"/></svg>',
  back: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"><path d="M15 6l-6 6 6 6"/></svg>',
  lock: '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4"><rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/></svg>',
  gear: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>',
  bat: (p, c) => `<svg width="22" height="14" viewBox="0 0 26 14" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="1" y="1" width="21" height="12" rx="3"/><rect x="3" y="3" width="${Math.max(1, 17 * p / 100)}" height="8" rx="1.5" fill="currentColor" stroke="none"/><path d="M24 5v4" stroke-linecap="round"/>${c ? '<path d="M12 2.5l-3 5h4l-2 4.5" stroke="var(--bg)" stroke-width="1.6"/>' : ""}</svg>`,
};
const openUri = (u) => { invoke("launch_app", { path: u }); invoke("popover_close"); };
const sigBars = (q) => (q >= 75 ? 4 : q >= 50 ? 3 : q >= 25 ? 2 : 1);

async function renderQuick() {
  const box = el("div", "qs");
  panel.append(box);
  let st = null;
  let wifi = null;          // { ok, networks, code }
  let view = "main";        // "main" | "wifi" | "audio"
  let dragging = false;
  const throttle = (fn, ms) => { let t = 0, last = null; return (...a) => { last = a; if (t) return; t = setTimeout(() => { t = 0; fn(...last); }, ms); }; };
  const setVol = throttle((v) => invoke("quick_volume", { volume: v, muted: null }), 60);
  const setDdc = throttle((id, v) => invoke("quick_brightness", { id, kind: "ddc", value: v }), 180);

  const radio = (k) => (st?.radios || []).find((r) => r.kind === k);
  const current = () => (wifi?.networks || []).find((n) => n.connected);

  function tile(id, icon, label, sub, on, opts = {}) {
    return `<div class="qt${on ? " on" : ""}${opts.disabled ? " off" : ""}" data-t="${id}">
      <button class="qt-main" title="${esc(opts.title || label)}">${icon}</button>
      ${opts.more ? `<button class="qt-more" title="${esc(opts.moreTitle || "")}">${QI.chev}</button>` : ""}
      <span class="qt-l">${esc(label)}</span><span class="qt-s">${esc(sub || "")}</span></div>`;
  }

  function drawMain() {
    const w = radio("wifi"), b = radio("bluetooth");
    const cur = current();
    const a = st?.audio || { volume: 0, muted: false, devices: [] };
    const dev = a.devices.find((d) => d.default);
    box.innerHTML = `
      <div class="qt-grid">
        ${tile("wifi", QI.wifi(cur ? sigBars(cur.signal) : 4), "Wi-Fi", w ? (w.on ? (cur ? cur.ssid : "Bağlı değil") : "Kapalı") : "Kablosuz kart yok", w?.on, { more: true, moreTitle: "Ağları göster", disabled: !w })}
        ${tile("bluetooth", QI.bt, "Bluetooth", b ? (b.on ? "Açık" : "Kapalı") : "Yok", b?.on, { more: true, moreTitle: "Bluetooth cihazları", disabled: !b })}
        ${tile("airplane", QI.plane, "Uçak modu", st?.airplane ? "Açık" : "Kapalı", st?.airplane)}
        ${tile("saver", QI.leaf, "Enerji tasarrufu", "Ayarlar", false, { title: "Windows ayarlarında aç" })}
        ${tile("night", QI.moon, "Gece ışığı", "Ayarlar", false, { title: "Windows ayarlarında aç" })}
        ${tile("hotspot", QI.hotspot, "Etkin nokta", "Ayarlar", false, { title: "Mobil etkin nokta ayarları" })}
      </div>
      <div class="qs-row vol">
        <button class="qs-ic mute" title="${a.muted ? "Sesi aç" : "Sesi kapat"}">${QI.vol(a.muted, a.volume)}</button>
        <input type="range" min="0" max="100" step="1" value="${Math.round(a.volume)}" class="qs-range" style="--p:${a.volume}%" aria-label="Ses">
        <span class="qs-v">${Math.round(a.volume)}</span>
        <button class="qs-ic devs" title="Çıkış aygıtı: ${esc(dev?.name || "–")}">${QI.chev}</button>
      </div>
      ${(st?.brightness || []).map((br) => `
        <div class="qs-row bri" data-id="${esc(br.id)}" data-kind="${br.kind}">
          <span class="qs-ic" title="${esc(br.title)}">${QI.sun}</span>
          <input type="range" min="0" max="100" step="1" value="${br.value}" class="qs-range" style="--p:${br.value}%" aria-label="Parlaklık ${esc(br.title)}">
          <span class="qs-v">${br.value}</span>
          ${(st.brightness.length > 1) ? `<span class="qs-mon">${esc(br.title.replace("Ekran ", ""))}</span>` : `<span class="qs-mon"></span>`}
        </div>`).join("")}
      <div class="qs-foot">
        ${st?.battery >= 0 ? `<span class="qs-bat${st.battery <= 20 && !st.charging ? " low" : ""}">${QI.bat(st.battery, st.charging)} %${st.battery}${st.charging ? " · şarjda" : ""}</span>` : "<span></span>"}
        <button class="qs-link win" title="Windows'un kendi hızlı ayarlar paneli">Windows paneli</button>
        <button class="qs-ic gear" title="Tüm ayarlar">${QI.gear}</button>
      </div>`;
    bindMain();
    relayout();
  }

  function bindMain() {
    box.querySelectorAll(".qt").forEach((t) => {
      const id = t.dataset.t;
      t.querySelector(".qt-main").onclick = async () => {
        if (id === "saver") return openUri("ms-settings:batterysaver");
        if (id === "night") return openUri("ms-settings:nightlight");
        if (id === "hotspot") return openUri("ms-settings:network-mobilehotspot");
        if (t.classList.contains("off")) return;
        const on = !t.classList.contains("on");
        t.classList.toggle("on", on);
        t.classList.add("busy");
        await invoke("quick_radio", { kind: id, on }).catch(() => {});
        await refresh();
      };
      const more = t.querySelector(".qt-more");
      if (more) more.onclick = () => (id === "wifi" ? showWifi() : openUri("ms-settings:bluetooth"));
    });
    const vol = box.querySelector(".vol .qs-range");
    vol.oninput = () => {
      dragging = true;
      vol.style.setProperty("--p", `${vol.value}%`);
      box.querySelector(".vol .qs-v").textContent = vol.value;
      box.querySelector(".mute").innerHTML = QI.vol(false, Number(vol.value));
      setVol(Number(vol.value));
    };
    vol.onchange = () => { dragging = false; };
    box.querySelector(".mute").onclick = async () => {
      await invoke("quick_volume", { volume: null, muted: !st.audio.muted });
      refresh();
    };
    box.querySelector(".devs").onclick = () => { view = "audio"; drawAudio(); };
    box.querySelectorAll(".bri").forEach((row) => {
      const r = row.querySelector(".qs-range");
      r.oninput = () => {
        dragging = true;
        r.style.setProperty("--p", `${r.value}%`);
        row.querySelector(".qs-v").textContent = r.value;
        if (row.dataset.kind === "ddc") setDdc(row.dataset.id, Number(r.value));
      };
      r.onchange = () => {
        dragging = false;
        invoke("quick_brightness", { id: row.dataset.id, kind: row.dataset.kind, value: Number(r.value) });
      };
    });
    box.querySelector(".win").onclick = () => { invoke("system_action", { action: "quick" }); invoke("popover_close"); };
    box.querySelector(".gear").onclick = () => openUri("ms-settings:");
  }

  function drawAudio() {
    const a = st.audio;
    box.innerHTML = `
      <div class="qs-head"><button class="qs-back">${QI.back}</button><strong>Ses çıkışı</strong></div>
      <div class="qs-list">${a.devices.map((d) => `
        <button class="qs-item${d.default ? " on" : ""}" data-id="${esc(d.id)}">${QI.vol(false, 60)}<span>${esc(d.name || "Ses aygıtı")}</span>${d.default ? '<em>Kullanılıyor</em>' : ""}</button>`).join("") || '<div class="qs-empty">Ses aygıtı bulunamadı.</div>'}</div>
      <div class="qs-foot"><span></span><button class="qs-link snd">Ses ayarları</button></div>`;
    box.querySelector(".qs-back").onclick = () => { view = "main"; drawMain(); };
    box.querySelector(".snd").onclick = () => openUri("ms-settings:sound");
    box.querySelectorAll(".qs-item").forEach((b) => (b.onclick = async () => {
      b.classList.add("busy");
      await invoke("quick_audio_device", { id: b.dataset.id }).catch(() => {});
      await refresh();
      drawAudio();
    }));
    relayout();
  }

  function drawWifi(scanning) {
    const w = radio("wifi");
    const nets = wifi?.networks || [];
    let list;
    if (!w?.on) list = '<div class="qs-empty">Wi-Fi kapalı.</div>';
    else if (wifi && !wifi.ok) list = wifi.code === 5
      ? '<div class="qs-empty">Ağ listesini görmek için Windows\'un uygulamalara konum izni vermesi gerekiyor.<br><button class="qs-link loc">Konum izni ayarları</button></div>'
      : `<div class="qs-empty">Ağlar okunamadı (kod ${wifi.code}).</div>`;
    else if (!nets.length) list = `<div class="qs-empty">${scanning ? "Ağlar aranıyor…" : "Ağ bulunamadı."}</div>`;
    else list = nets.map((n) => `
      <button class="qs-item net${n.connected ? " on" : ""}" data-p="${esc(n.profile)}" data-ssid="${esc(n.ssid)}" data-sec="${n.secured ? 1 : 0}">
        ${QI.wifi(sigBars(n.signal))}<span>${esc(n.ssid)} ${n.secured ? QI.lock : ""}</span>
        <em>${n.connected ? "Bağlı · kes" : n.profile ? "Bağlan" : n.secured ? "Şifre gerekir" : "Bağlan"}</em>
      </button>`).join("");
    box.innerHTML = `
      <div class="qs-head"><button class="qs-back">${QI.back}</button><strong>Wi-Fi</strong>${scanning ? '<span class="qs-spin"></span>' : ""}
        <label class="qs-sw"><input type="checkbox" ${w?.on ? "checked" : ""}><span></span></label></div>
      <div class="qs-list nets">${list}</div>
      <div class="qs-foot"><span></span><button class="qs-link more">Diğer ağ ayarları</button></div>`;
    box.querySelector(".qs-back").onclick = () => { view = "main"; drawMain(); };
    box.querySelector(".more").onclick = () => openUri("ms-settings:network-wifi");
    box.querySelector(".loc")?.addEventListener("click", () => openUri("ms-settings:privacy-location"));
    box.querySelector(".qs-sw input").onchange = async (e) => {
      await invoke("quick_radio", { kind: "wifi", on: e.target.checked }).catch(() => {});
      await refresh();
      showWifi();
    };
    box.querySelectorAll(".net").forEach((b) => (b.onclick = async () => {
      if (b.classList.contains("on")) {
        b.querySelector("em").textContent = "Kesiliyor…";
        await invoke("wifi_disconnect");
      } else if (b.dataset.p) {
        b.querySelector("em").textContent = "Bağlanıyor…";
        await invoke("wifi_connect", { profile: b.dataset.p });
        await new Promise((r) => setTimeout(r, 2500));
      } else {
        // Yeni şifreli ağ: şifre Windows'un kendi ekranında girilir
        return openUri("ms-availablenetworks:");
      }
      wifi = await invoke("wifi_list", { scan: false }).catch(() => wifi);
      drawWifi(false);
    }));
    relayout();
  }

  async function showWifi() {
    view = "wifi";
    drawWifi(true);
    wifi = await invoke("wifi_list", { scan: false }).catch(() => wifi);
    if (view === "wifi") drawWifi(true);
    wifi = await invoke("wifi_list", { scan: true }).catch(() => wifi);
    if (view === "wifi") drawWifi(false);
  }

  async function refresh() {
    st = (await invoke("quick_state").catch(() => st)) || st;
    if (view === "main" && !dragging) drawMain();
  }

  box.innerHTML = '<div class="qs-empty">Yükleniyor…</div>';
  relayout();
  [st, wifi] = await Promise.all([invoke("quick_state").catch(() => null), invoke("wifi_list", { scan: false }).catch(() => null)]);
  st = st || { audio: { volume: 0, muted: false, devices: [] }, radios: [], brightness: [], battery: -1 };
  drawMain();
  stopLive();
  liveTimer = setInterval(() => { if (spec?.kind !== "quick") return stopLive(); if (view === "main") refresh(); }, 2500);
}

// ---------------------------------------------------------------------------
// Klasör yığını: son dosyalar (ızgara ya da yelpaze)
// ---------------------------------------------------------------------------
const fileAgo = (ms) => {
  const m = Math.round((Date.now() - ms) / 60000);
  if (m < 60) return `${Math.max(m, 1)} dk`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} sa`;
  const d = Math.round(h / 24);
  return d < 30 ? `${d} gün` : new Date(ms).toLocaleDateString("tr-TR", { day: "numeric", month: "short" });
};
const EXT_COLOR = { pdf: "#E5484D", doc: "#2F7BFF", docx: "#2F7BFF", xls: "#1E9E63", xlsx: "#1E9E63", csv: "#1E9E63", ppt: "#F97316", pptx: "#F97316", zip: "#A16207", rar: "#A16207", "7z": "#A16207", exe: "#64748B", msi: "#64748B", txt: "#64748B", png: "#7C5CFF", jpg: "#7C5CFF", jpeg: "#7C5CFF", mp4: "#EC4899", mp3: "#EC4899" };
function fileGlyph(e) {
  if (e.dir) return '<svg viewBox="0 0 40 34" width="40" height="34"><path d="M3 7a4 4 0 0 1 4-4h8l4 4h14a4 4 0 0 1 4 4v16a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4z" fill="#5B9BFF"/></svg>';
  const ext = (e.name.split(".").pop() || "").toLowerCase();
  return `<span class="fg" style="--fc:${EXT_COLOR[ext] || "#64748B"}">${esc(ext.slice(0, 4).toUpperCase())}</span>`;
}

async function renderStack(st) {
  let sort = st.sort || "date";
  let view = st.view || "grid";
  const FAN = 9; // yelpazede aynı anda görünen öğe
  let fanOff = 0; // yelpazede kaydırma (en yeniden kaç öğe geride)
  let list = [];
  const box = el("div", "stk");
  panel.append(box);

  const save = () => invoke("stack_prefs", { path: st.path, view, sort }).catch(() => {});

  function fanPlace(body, dir) {
    // En yeni en altta (dock'a yakın); kaydırınca eskiler yukarıdan gelir
    const shown = list.slice(fanOff, fanOff + FAN).reverse();
    const nodes = shown.map((e, i, arr) => itemFor(e, arr.length - 1 - i));
    body.replaceChildren(...nodes);
    if (dir) {
      body.classList.remove("up", "down");
      void body.offsetWidth;
      body.classList.add(dir);
    }
    const more = box.querySelector(".stk-more");
    if (more) {
      more.hidden = list.length <= FAN;
      more.textContent = list.length > FAN ? `${fanOff + 1}–${Math.min(list.length, fanOff + FAN)} / ${list.length} · tekerlek ya da ↑ ↓ ile kaydır` : "";
    }
    thumbs(nodes);
  }

  function fanScroll(d) {
    const max = Math.max(0, list.length - FAN);
    const n = Math.max(0, Math.min(max, fanOff + d));
    if (n === fanOff) return;
    fanOff = n;
    fanPlace(box.querySelector(".stk-body"), d > 0 ? "up" : "down");
  }

  function itemFor(e, k) {
    const it = el("button", "stk-it");
    it.title = `${e.name}\n${fileAgo(e.modified)} önce`;
    it.innerHTML = `<span class="stk-th">${fileGlyph(e)}</span><span class="stk-nm"></span>${view === "grid" ? `<small>${fileAgo(e.modified)}</small>` : ""}`;
    it.querySelector(".stk-nm").textContent = e.name;
    it._e = e;
    if (view === "fan") {
      it.style.setProperty("--k", k);
      it.style.animationDelay = `${k * 22}ms`;
    } else {
      it.style.animationDelay = `${Math.min(k, 24) * 24 + 40}ms`;
      it.addEventListener("pointerdown", (ev) => {
        const r = it.getBoundingClientRect();
        const w = el("i", "rip");
        w.style.left = `${ev.clientX - r.left}px`;
        w.style.top = `${ev.clientY - r.top}px`;
        it.append(w);
        setTimeout(() => w.remove(), 600);
      });
    }
    it.onclick = () => { invoke("launch_app", { path: e.path }); invoke("popover_close"); };
    return it;
  }

  // Küçük resimler (aynı anda en çok 4 istek)
  function thumbs(nodes) {
    const q = nodes.filter((n) => !n._e.dir);
    const worker = async () => {
      while (q.length) {
        const it = q.shift();
        const u = await invoke("file_thumb", { path: it._e.path, size: view === "fan" ? 64 : 96 }).catch(() => null);
        if (u && it.isConnected) it.querySelector(".stk-th").innerHTML = `<img src="${u}" alt="">`;
      }
    };
    Promise.all([worker(), worker(), worker(), worker()]);
  }

  async function draw() {
    list = (await invoke("stack_list", { path: st.path, sort, limit: 60 }).catch(() => [])) || [];
    fanOff = 0;
    box.className = `stk ${view}`;
    box.innerHTML = `
      <div class="stk-head"><strong>${esc(st.name)}</strong><span>${list.length ? list.length + (list.length >= 60 ? "+" : "") + " öğe" : ""}</span>
        <div class="stk-seg"><button data-s="date" class="${sort === "date" ? "on" : ""}">Tarih</button><button data-s="name" class="${sort === "name" ? "on" : ""}">Ad</button></div>
        <div class="stk-seg"><button data-v="grid" class="${view === "grid" ? "on" : ""}" title="Izgara">▦</button><button data-v="fan" class="${view === "fan" ? "on" : ""}" title="Yelpaze">◜</button></div>
      </div>
      <div class="stk-body" tabindex="-1"></div>
      ${view === "fan" ? '<div class="stk-more" hidden></div>' : ""}
      <div class="stk-foot"><button class="stk-open">Klasörü aç</button></div>`;
    const body = box.querySelector(".stk-body");
    if (!list.length) body.innerHTML = '<div class="stk-empty">Bu klasör boş.</div>';
    else if (view === "fan") fanPlace(body);
    else {
      const nodes = list.map((e, i) => itemFor(e, i));
      body.append(...nodes);
      thumbs(nodes);
    }
    box.querySelectorAll("[data-s]").forEach((b) => (b.onclick = () => { sort = b.dataset.s; save(); draw(); }));
    box.querySelectorAll("[data-v]").forEach((b) => (b.onclick = () => { view = b.dataset.v; save(); draw(); }));
    box.querySelector(".stk-open").onclick = () => { invoke("launch_app", { path: "explorer.exe", args: `"${st.path}"` }); invoke("popover_close"); };
    relayout();
    body.focus({ preventScroll: true });
  }

  // Yelpaze: tekerlek ve ok tuşlarıyla kaydırma
  let wheelAcc = 0;
  box.addEventListener("wheel", (e) => {
    if (view !== "fan") return;
    e.preventDefault();
    wheelAcc += e.deltaY;
    while (Math.abs(wheelAcc) >= 60) {
      const d = wheelAcc < 0 ? 1 : -1; // yukarı = daha eskiler
      wheelAcc -= wheelAcc < 0 ? -60 : 60;
      fanScroll(d);
    }
  }, { passive: false });
  window.addEventListener("keydown", function onKey(e) {
    if (!box.isConnected) { window.removeEventListener("keydown", onKey); return; }
    if (view !== "fan") return;
    if (e.key === "ArrowUp") { e.preventDefault(); fanScroll(1); }
    else if (e.key === "ArrowDown") { e.preventDefault(); fanScroll(-1); }
    else if (e.key === "PageUp") { e.preventDefault(); fanScroll(FAN); }
    else if (e.key === "PageDown") { e.preventDefault(); fanScroll(-FAN); }
  });
  await draw();
}

// ---------------------------------------------------------------------------
// Başlangıç
// ---------------------------------------------------------------------------
(async () => {
  const pending = await invoke("popover_pending").catch(() => null);
  if (pending) show(pending);
})();

// Tarayıcıda önizleme (popover.html?kind=calendar | windows)
async function mockInvoke(cmd, args) {
  const q = new URLSearchParams(location.search);
  if (cmd === "get_config") return { theme: q.get("theme") || "dark", backdrop: "acrylic", accent: "#2F7BFF", dynamic: { enabled: q.has("wall"), tint: 45 }, weather: { enabled: true, auto_location: true, city: "İstanbul", lat: 41, lon: 29 } };
  if (cmd === "popover_pending") {
    setTimeout(() => panel.classList.add("in"), 50);
    if (q.get("kind") === "windows")
      return {
        kind: "windows", serial: 1, key: "chrome",
        payload: {
          name: "Chrome", icon: null,
          windows: [
            { hwnd: 1, title: "Cam Dock — GitHub", minimized: false },
            { hwnd: 2, title: "Google Takvim — Ekim 2026", minimized: false },
            { hwnd: 3, title: "YouTube", minimized: true },
          ],
        },
      };
    if (q.get("kind") === "tray") return { kind: "tray", serial: 1, key: "tray", payload: null };
    if (q.get("kind") === "stack") return { kind: "stack", serial: 1, key: "dl", payload: { name: "İndirilenler", path: "C:\\Users\\x\\Downloads", view: q.get("view") || "grid", sort: "date" } };
    if (q.get("kind") === "quick") return { kind: "quick", serial: 1, key: "quick", payload: null };
    if (q.get("kind") === "media") return { kind: "media", serial: 1, key: "media", payload: { exclude: "Spotify.exe#0" } };
    if (q.get("kind") === "clipboard") return { kind: "clipboard", serial: 1, key: "clipboard", payload: null };
    if (q.get("kind") === "claude") return { kind: "claude", serial: 1, key: "claude", payload: null };
    if (q.get("kind") === "weather") return { kind: "weather", serial: 1, key: "weather", payload: null };
    if (q.get("kind") === "usage") return { kind: "usage", serial: 1, key: "usage", payload: null };
    if (q.get("kind") === "news") return { kind: "news", serial: 1, key: "news", payload: { markets: ["USD", "EUR", "gram-altin"], feeds: [], focus: null } };
    return { kind: "calendar", serial: 1, key: "cal", payload: null };
  }
  if (cmd === "usage_data") {
    const hours = [0,0,0,0,0,0,0,0,1200,3100,3400,2900,1800,3300,3550,2700,1500,0,0,0,0,0,0,0];
    const rnd = (b, a) => Math.max(0, Math.min(100, b + Math.sin(Date.now() / 4000) * a + (Math.random() - .5) * a));
    const history = Array.from({ length: 120 }, (_, i) => ({ cpu: 18 + 14 * Math.sin(i / 9) + Math.random() * 8, ram: 61 + Math.sin(i / 30) * 2, gpu: 8 + 20 * Math.max(0, Math.sin(i / 14)), disk: Math.random() < .15 ? 30 + Math.random() * 40 : Math.random() * 5 }));
    return {
      screen: {
        date: "2026-10-30", seconds: hours.reduce((a, b) => a + b, 0), hours, first: "08:41", switches: 137,
        history: [["2026-10-24", 15800], ["2026-10-25", 9200], ["2026-10-26", 23500], ["2026-10-27", 26100], ["2026-10-28", 21400], ["2026-10-29", 24800]].map(([date, seconds]) => ({ date, seconds })),
        apps: {
          a: { name: "Chrome", path: "C:\\chrome.exe", seconds: 8400 }, b: { name: "Code", path: "C:\\code.exe", seconds: 6200 },
          c: { name: "Excel", path: "C:\\excel.exe", seconds: 3900 }, d: { name: "Teams", path: "C:\\teams.exe", seconds: 2400 },
          e: { name: "Spotify", path: "C:\\spotify.exe", seconds: 1300 }, f: { name: "Telegram", path: "", seconds: 700 }, g: { name: "Explorer", path: "", seconds: 550 },
        },
      },
      now: { cpu: rnd(24, 10), ram: 62, ram_used_gb: 19.8, ram_total_gb: 31.9, gpu: rnd(12, 10), disk: rnd(4, 4) },
      history,
      info: { uptime_sec: 273000, disk_free_gb: 312, disk_total_gb: 951, battery: -1, charging: false, procs: 284, cores: 16, cpu_name: "AMD Ryzen 7 7840HS w/ Radeon 780M Graphics", os: "Windows 11 25H2 · derleme 26200" },
    };
  }
  if (cmd === "theme_dark") return (q.get("theme") || "dark") === "dark";
  if (cmd === "stack_list") return ["Fatura-Ekim.pdf", "Ekran görüntüsü 2026-11-02.png", "Kurulum.exe", "Sunum.pptx", "Tatil fotoğrafı.jpg", "Veri analizi.xlsx", "Notlar.txt", "Arşiv.zip", "Proje", "Rapor taslağı.docx", "Video.mp4", "Müzik.mp3"]
    .slice(0, args.limit || 60).map((n, i) => ({ name: n, path: "C:\\x\\" + n, dir: !n.includes("."), modified: Date.now() - i * 3600e3 * 7, size: 1e6 }));
  if (cmd === "file_thumb") return /png|jpg/.test(args.path) ? MOCK.wallpaper(args.path.includes("png") ? "ocean" : "sunset").image : null;
  if (cmd === "quick_state") return {
    audio: { volume: 64, muted: false, devices: [{ id: "a", name: "Hoparlör (Realtek Audio)", default: true }, { id: "b", name: "AirPods Pro", default: false }, { id: "c", name: "DELL U2723QE (HDMI)", default: false }] },
    radios: [{ kind: "wifi", name: "Wi-Fi", on: true }, { kind: "bluetooth", name: "Bluetooth", on: q.get("bt") === "1" }],
    brightness: [{ id: "1", title: "Ekran 1", kind: "wmi", value: 72 }, { id: "2", title: "Ekran 2", kind: "ddc", value: 45 }],
    battery: 78, charging: true, airplane: false,
  };
  if (cmd === "wifi_list") return { ok: true, networks: [
    { ssid: "Ev_5G", signal: 92, secured: true, connected: true, profile: "Ev_5G" },
    { ssid: "Ofis-Misafir", signal: 61, secured: false, connected: false, profile: "" },
    { ssid: "TurkTelekom_ZTF4", signal: 44, secured: true, connected: false, profile: "" },
    { ssid: "Kafe Wi-Fi", signal: 20, secured: true, connected: false, profile: "Kafe Wi-Fi" },
  ] };
  if (cmd === "media_list") return [
    { id: "Spotify.exe#0", active: true, title: "Gece Sürüşü", artist: "Örnek Sanatçı", playing: true, position_ms: 1, duration_ms: 2, source: "Spotify.exe", track_key: "a" },
    { id: "Chrome#0", active: true, title: "Veri ambarı mimarisi — konferans kaydı", artist: "YouTube", playing: false, position_ms: 600000, duration_ms: 2400000, source: "Chrome", track_key: "b" },
    { id: "Edge#0", active: true, title: "Lo-fi çalışma listesi", artist: "YouTube Music", playing: true, position_ms: 90000, duration_ms: 200000, source: "msedge.exe", track_key: "c" },
  ];
  if (cmd === "clip_list") return [
    { id: 1, kind: "text", text: "SELECT region, SUM(revenue) FROM sales\nWHERE year = 2026\nGROUP BY region;", source: "Code", source_path: "", time: Date.now() - 40000, pinned: true },
    { id: 2, kind: "image", preview: MOCK.wallpaper("ocean").image, width: 1920, height: 1080, source: "SnippingTool", source_path: "", time: Date.now() - 300000, pinned: false, text: "" },
    { id: 3, kind: "text", text: "https://github.com/aliapaydin/cam-dock/actions", source: "Chrome", source_path: "", time: Date.now() - 900000, pinned: false },
    { id: 4, kind: "text", text: "Toplantı notları: Q4 hedefleri, yeni dashboard taslağı, veri ambarı geçişi takvimi.", source: "Notepad", source_path: "", time: Date.now() - 3600000, pinned: false },
  ];
  if (cmd === "claude_usage") return { state: q.get("cstate") || "ok", plan: "Max", updated: Date.now() - 120000, message: "", limits: q.get("cstate") === "no_claude_code" ? [] : [
    { label: "Oturum", percent: 34, resets_at: new Date(Date.now() + 2.3 * 3600e3).toISOString() },
    { label: "Haftalık", percent: 58, resets_at: new Date(Date.now() + 3.4 * 86400e3).toISOString() },
    { label: "Opus", percent: 87, resets_at: new Date(Date.now() + 3.4 * 86400e3).toISOString() },
  ] };
  if (cmd === "wallpaper") return MOCK.wallpaper(q.get("wall"));
  if (cmd === "fetch_text") return /open-meteo|geojs/.test(args.url) ? MOCK.fetch(args.url) : mockFetch(args.url);
  if (cmd === "tray_apps")
    return ["Steam", "Discord", "OneDrive", "Telegram", "NVIDIA", "Bluetooth", "Docker Desktop", "PowerToys", "Logi Options+", "1Password"].map((n, i) => ({ name: n, exe: `C:\\${n}.exe`, icon: null, promoted: i < 2 }));
  return null;
}

function mockFetch(url) {
  if (url.includes("truncgil")) return JSON.stringify({
    Update_Date: "2026-10-30 15:45:03",
    USD: { "Alış": "49,9424", "Satış": "49,9531", "Değişim": "%0,08" }, EUR: { "Alış": "57,9010", "Satış": "57,9480", "Değişim": "%0,21" },
    GBP: { "Alış": "66,3120", "Satış": "66,4011", "Değişim": "%-0,12" }, CHF: { "Alış": "62,0100", "Satış": "62,1033", "Değişim": "%0,05" },
    JPY: { "Alış": "0,3271", "Satış": "0,3289", "Değişim": "%-0,30" }, SAR: { "Alış": "13,3010", "Satış": "13,3200", "Değişim": "%0,07" },
    "gram-altin": { "Alış": "6.355,43", "Satış": "6.356,31", "Değişim": "%-1,49" }, "ceyrek-altin": { "Alış": "10.390,00", "Satış": "10.512,00", "Değişim": "%-1,52" },
    "yarim-altin": { "Alış": "20.770,00", "Satış": "21.020,00", "Değişim": "%-1,52" }, "tam-altin": { "Alış": "41.300,00", "Satış": "41.800,00", "Değişim": "%-1,50" },
    "cumhuriyet-altini": { "Alış": "42.100,00", "Satış": "42.650,00", "Değişim": "%-1,48" }, ons: { "Alış": "$3.957,67", "Satış": "$3.958,22", "Değişim": "%-1,57" },
    gumus: { "Alış": "80,31", "Satış": "80,39", "Değişim": "%-3,98" },
  });
  const titles = ["Merkez Bankası faiz kararını açıkladı: piyasalar beklentiye odaklandı", "İstanbul'da yeni metro hattı hafta sonu hizmete giriyor", "Avrupa borsaları günü karışık seyirle tamamladı",
    "Yerli elektrikli otomobilin yeni modeli tanıtıldı", "Milli takımın aday kadrosu belli oldu", "Hafta sonu için kuvvetli yağış uyarısı", "Teknoloji şirketlerinden üçüncü çeyrek bilançoları",
    "Üniversitelerde yeni dönem burs başvuruları başladı", "Akdeniz'de yeni doğal gaz keşfi iddiası", "Yapay zekâ düzenlemesi için yeni taslak meclise sunuldu"];
  const src = ["Anadolu Ajansı", "NTV", "BBC Türkçe", "Hürriyet", "Dünya", "Webrazzi"];
  return `<rss><channel><title>Google Haberler</title>${titles.map((t, i) => `<item><title>${t} - ${src[i % 6]}</title><link>https://example.com/${i}</link><pubDate>${new Date(Date.now() - (i * 23 + 4) * 60000).toUTCString()}</pubDate><source>${src[i % 6]}</source></item>`).join("")}</channel></rss>`;
}
