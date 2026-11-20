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
  else if (s.kind === "notes") await renderNotes();
  else if (s.kind === "todo") await renderTodo();
  else if (s.kind === "focus") await renderFocus();
  else if (s.kind === "world") renderWorld();
  else if (s.kind === "net") renderNet();
  else if (s.kind === "disk") await renderDisk();
  else if (s.kind === "crypto") await renderCrypto();
  else if (s.kind === "prayer") await renderPrayer();
  else if (s.kind === "countdown") renderCountdown();
  else if (s.kind === "calc") renderCalc();
  else if (s.kind === "color") await renderColor();
  else if (s.kind === "translate") await renderTranslate();
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
      t.title = a.tip && a.tip !== a.name ? `${a.name}\n${a.tip}` : `${a.name}\n${a.exe}`;
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
      t.onclick = async (e) => {
        ripple(e);
        // Taskbar'daki gibi simgeye tıklandığını uygulamaya bildir; simge
        // bilinmiyorsa uygulamayı yeniden başlat (çoğu tek örnekli: penceresi açılır).
        const ok = a.id ? await invoke("tray_click", { id: a.id }).catch(() => false) : false;
        if (!ok) invoke("launch_app", { path: a.exe });
        setTimeout(() => invoke("popover_close"), 140);
      };
      // Sağ tık: taskbar'daki gibi uygulamanın kendi tepsi menüsü. Simgesi
      // henüz bilinmiyorsa (yeni açılmış) dock'un uygulama menüsü.
      t.oncontextmenu = async (e) => {
        e.preventDefault();
        ripple(e);
        const ok = await invoke("tray_menu", { exe: a.exe, id: a.id || null }).catch(() => false);
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
// Yapışkan notlar
// ---------------------------------------------------------------------------
const NOTE_COLORS = { yellow: "#FFE36E", pink: "#FF9EC8", blue: "#8EC5FF", green: "#8EE39A", purple: "#B79BFF", orange: "#FFB070", gray: "#3A3F4C" };
const PI = {
  desk: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 21h8M12 17v4"/><path d="M14 7.5h4v4h-4z" fill="currentColor"/></svg>',
  del: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/></svg>',
  plus: '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>',
  pal: '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3a9 9 0 100 18c1.1 0 1.6-.9 1.2-1.8-.5-1-.1-2.2 1.2-2.2H17a4 4 0 004-4c0-5.5-4-10-9-10z"/><circle cx="7.5" cy="11" r="1.2" fill="currentColor"/><circle cx="10.5" cy="7" r="1.2" fill="currentColor"/><circle cx="15" cy="7.5" r="1.2" fill="currentColor"/></svg>',
};
let notesUnlisten = null;
async function renderNotes() {
  const box = el("div", "nts");
  box.innerHTML = `
    <div class="clp-head"><strong>Yapışkan notlar</strong><span class="clp-n"></span>
      <button class="nts-all"></button>
      <button class="nts-new">${PI.plus} Yeni not</button></div>
    <div class="nts-grid"></div>
    <div class="clp-foot">Notu masaüstüne bırak: istediğin köşeye sürükle, kenara yaklaşınca yaslanır. Üst şeritten taşı, sağ alt köşeden büyüt.</div>`;
  panel.append(box);
  const grid = box.querySelector(".nts-grid");
  let editing = null; // düzenlenen notun kimliği
  let list = [];
  const saveT = new Map();

  function card(n, i) {
    const c = el("div", "nt" + (n.desktop ? " desk" : "") + (editing === n.id ? " editing" : ""));
    c.style.setProperty("--p", NOTE_COLORS[n.color] || NOTE_COLORS.yellow);
    c.style.setProperty("--i", i);
    c.dataset.color = n.color;
    c.innerHTML = `
      <div class="nt-body"></div>
      <div class="nt-pal">${Object.entries(NOTE_COLORS).map(([k, v]) => `<button data-c="${k}" style="background:${v}" class="${k === n.color ? "on" : ""}" aria-label="${k}"></button>`).join("")}</div>
      <div class="nt-acts">
        <button class="nt-act desk ${n.desktop ? "on" : ""}" title="${n.desktop ? "Masaüstünden kaldır" : "Masaüstüne bırak"}">${PI.desk}</button>
        <button class="nt-act pal" title="Renk">${PI.pal}</button>
        <button class="nt-act del" title="Sil">${PI.del}</button>
      </div>`;
    const body = c.querySelector(".nt-body");
    if (editing === n.id) {
      const ta = el("textarea", "nt-ta");
      ta.value = n.text;
      ta.placeholder = "Bir şey yaz…";
      ta.oninput = () => {
        clearTimeout(saveT.get(n.id));
        saveT.set(n.id, setTimeout(() => invoke("note_update", { id: n.id, text: ta.value }).catch(() => {}), 300));
      };
      ta.onblur = () => setTimeout(() => { if (editing === n.id && !c.contains(document.activeElement)) { editing = null; draw(); } }, 120);
      body.append(ta);
      setTimeout(() => { ta.focus(); ta.setSelectionRange(ta.value.length, ta.value.length); }, 30);
    } else {
      body.textContent = n.text || "Boş not — yazmak için tıkla";
      body.classList.toggle("empty", !n.text);
      body.onclick = () => { editing = n.id; draw(); };
    }
    c.querySelector(".desk").onclick = () => invoke("note_desktop", { id: n.id, on: !n.desktop }).catch(() => {});
    c.querySelector(".pal").onclick = () => c.classList.toggle("pal-open");
    c.querySelectorAll(".nt-pal button").forEach((b) => b.onclick = () => {
      c.classList.remove("pal-open");
      invoke("note_update", { id: n.id, color: b.dataset.c }).catch(() => {});
    });
    const del = c.querySelector(".del");
    del.onclick = () => {
      if (!del.classList.contains("arm")) {
        del.classList.add("arm");
        del.title = "Silmek için tekrar tıkla";
        setTimeout(() => del.classList.remove("arm"), 2500);
        return;
      }
      c.classList.add("bye");
      setTimeout(() => invoke("note_delete", { id: n.id }).catch(() => {}), 220);
    };
    return c;
  }

  function draw() {
    box.querySelector(".clp-n").textContent = list.length ? `${list.length} not` : "";
    const desk = list.filter((n) => n.desktop).length;
    const all = box.querySelector(".nts-all");
    all.style.display = list.length ? "" : "none";
    all.innerHTML = `${PI.desk} ${desk === list.length ? "Masaüstünden kaldır" : "Hepsi masaüstüne"}`;
    all.onclick = () => invoke("notes_desktop_all", { on: desk !== list.length }).catch(() => {});
    if (!list.length) {
      grid.innerHTML = `<div class="clp-empty nts-empty"><span class="nts-ill"><i></i><i></i><i></i></span>Henüz not yok.<br>Yeni bir not yaz, istersen masaüstünün bir köşesine bırak.</div>`;
    } else grid.replaceChildren(...list.map(card));
    relayout();
  }
  box.querySelector(".nts-new").onclick = async () => {
    const n = await invoke("note_new", {}).catch(() => null);
    if (n) { editing = n.id; list = [n, ...list.filter((x) => x.id !== n.id)]; draw(); }
  };
  list = (await invoke("notes_list").catch(() => [])) || [];
  draw();
  notesUnlisten?.();
  notesUnlisten = await listen("notes-changed", (e) => {
    if (spec?.kind !== "notes") return;
    list = [...(e.payload || [])].sort((a, b) => b.updated - a.updated);
    // Yazarken kartı yeniden çizme (imleç kaçmasın)
    if (editing && document.activeElement?.classList.contains("nt-ta")) {
      const cur = list.find((n) => n.id === editing);
      const c = document.activeElement.closest(".nt");
      if (cur && c) { c.style.setProperty("--p", NOTE_COLORS[cur.color]); c.dataset.color = cur.color; }
      return;
    }
    draw();
  });
}

// ---------------------------------------------------------------------------
// Yapılacaklar
// ---------------------------------------------------------------------------
let todoUnlisten = null;
async function renderTodo() {
  const box = el("div", "tdo");
  box.innerHTML = `
    <div class="clp-head"><strong>Yapılacaklar</strong><span class="clp-n"></span><button class="clp-clear">Tamamlananları temizle</button></div>
    <div class="tdo-bar"><i></i></div>
    <form class="tdo-add"><span class="tdo-plus">${PI.plus}</span><input placeholder="Yeni görev yaz, Enter'a bas" maxlength="200" aria-label="Yeni görev"></form>
    <div class="tdo-list"></div>`;
  panel.append(box);
  const listEl = box.querySelector(".tdo-list");
  const input = box.querySelector("input");
  let items = [];
  let fresh = null; // yeni eklenenin metni: giriş animasyonu için
  box.querySelector("form").onsubmit = (e) => {
    e.preventDefault();
    const t = input.value.trim();
    if (!t) return;
    fresh = t;
    input.value = "";
    invoke("todo_add", { text: t }).catch(() => {});
  };
  box.querySelector(".clp-clear").onclick = () => {
    listEl.querySelectorAll(".td.done").forEach((r) => r.classList.add("bye"));
    setTimeout(() => invoke("todo_clear_done").catch(() => {}), 220);
  };

  function row(t) {
    const r = el("div", "td" + (t.done ? " done" : ""));
    r.dataset.id = t.id;
    r.innerHTML = `<button class="td-check" aria-label="${t.done ? "Geri al" : "Tamamla"}"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3.4" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg></button>
      <span class="td-text"></span><button class="td-x" aria-label="Sil">${SVG.x}</button>`;
    const tx = r.querySelector(".td-text");
    tx.textContent = t.text;
    if (fresh && t.text === fresh && !t.done) { r.classList.add("new"); fresh = null; }
    r.querySelector(".td-check").onclick = () => {
      r.classList.toggle("done");
      if (!t.done) r.classList.add("pop");
      setTimeout(() => invoke("todo_toggle", { id: t.id }).catch(() => {}), t.done ? 0 : 420);
    };
    r.querySelector(".td-x").onclick = () => { r.classList.add("bye"); setTimeout(() => invoke("todo_delete", { id: t.id }).catch(() => {}), 200); };
    tx.ondblclick = () => {
      tx.contentEditable = "plaintext-only";
      tx.focus();
      document.getSelection()?.selectAllChildren(tx);
      const end = () => { tx.contentEditable = "false"; invoke("todo_edit", { id: t.id, text: tx.textContent || "" }).catch(() => {}); };
      tx.onblur = end;
      tx.onkeydown = (e) => { if (e.key === "Enter") { e.preventDefault(); tx.blur(); } if (e.key === "Escape") { tx.textContent = t.text; tx.blur(); } };
    };
    // Basılı tutup sürükleyerek sırala
    r.addEventListener("pointerdown", (e) => {
      if (e.button !== 0 || e.target.closest("button") || tx.isContentEditable) return;
      const y0 = e.clientY;
      let drag = null;
      const hold = setTimeout(() => start(), 220);
      const start = () => {
        const rows = [...listEl.querySelectorAll(".td")];
        drag = { rows, from: rows.indexOf(r), to: rows.indexOf(r), h: r.offsetHeight + 6, mids: rows.map((x) => x.offsetTop + x.offsetHeight / 2) };
        r.classList.add("lift");
        listEl.classList.add("sorting");
      };
      const move = (ev) => {
        if (!drag) { if (Math.abs(ev.clientY - y0) > 6) { clearTimeout(hold); start(); } else return; }
        const dy = ev.clientY - y0;
        r.style.transform = `translateY(${dy}px) scale(1.02)`;
        const cy = drag.mids[drag.from] + dy;
        let to = 0;
        drag.mids.forEach((m, i) => { if (i !== drag.from && m < cy) to++; });
        drag.to = to;
        drag.rows.forEach((x, i) => {
          if (x === r) return;
          let sh = 0;
          if (drag.from < to && i > drag.from && i <= to) sh = -drag.h;
          if (drag.from > to && i >= to && i < drag.from) sh = drag.h;
          x.style.transform = sh ? `translateY(${sh}px)` : "";
        });
      };
      const up = () => {
        clearTimeout(hold);
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        if (!drag) return;
        listEl.classList.remove("sorting");
        r.classList.remove("lift");
        const ids = drag.rows.map((x) => x.dataset.id);
        const [m] = ids.splice(drag.from, 1);
        ids.splice(drag.to, 0, m);
        drag.rows.forEach((x) => (x.style.transform = ""));
        if (drag.to !== drag.from) invoke("todo_order", { ids }).catch(() => {});
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    });
    return r;
  }

  function draw() {
    const done = items.filter((t) => t.done).length;
    box.querySelector(".clp-n").textContent = items.length ? `${done}/${items.length}` : "";
    box.querySelector(".clp-clear").style.visibility = done ? "visible" : "hidden";
    box.querySelector(".tdo-bar i").style.width = items.length ? `${(done / items.length) * 100}%` : "0";
    box.querySelector(".tdo-bar").classList.toggle("full", items.length > 0 && done === items.length);
    if (!items.length) listEl.innerHTML = `<div class="clp-empty">Liste boş. Yukarıya yaz, Enter'a bas.</div>`;
    else listEl.replaceChildren(...items.map(row));
    relayout();
  }
  items = (await invoke("todos_list").catch(() => [])) || [];
  draw();
  setTimeout(() => input.focus(), 60);
  todoUnlisten?.();
  todoUnlisten = await listen("todos-changed", (e) => {
    if (spec?.kind !== "todo") return;
    items = e.payload || [];
    if (!document.activeElement?.isContentEditable) draw();
  });
}

// ---------------------------------------------------------------------------
// Odak (Pomodoro)
// ---------------------------------------------------------------------------
let focusUnlisten = null;
async function renderFocus() {
  const NAMES = { idle: "Odaklanmaya hazır", focus: "Odak", short: "Kısa mola", long: "Uzun mola" };
  const box = el("div", "fcs");
  const R = 54, C = 2 * Math.PI * R;
  box.innerHTML = `
    <div class="fcs-top">
      <div class="fcs-ring">
        <svg viewBox="0 0 128 128" width="128" height="128"><circle cx="64" cy="64" r="${R}" class="trk"/><circle cx="64" cy="64" r="${R}" class="val" stroke-dasharray="${C}"/></svg>
        <div class="fcs-mid"><b class="fcs-time"></b><span class="fcs-phase"></span></div>
      </div>
      <div class="fcs-side">
        <div class="fcs-dots"></div>
        <div class="fcs-btns">
          <button class="fcs-main"></button>
          <button class="fcs-b skip" title="Sıradaki aşamaya geç"><svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><path d="M5 5.5v13l9-6.5zM15 5h3v14h-3z"/></svg></button>
          <button class="fcs-b reset" title="Sıfırla"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12a8 8 0 1 0 2.5-5.8"/><path d="M4 4v4h4"/></svg></button>
        </div>
        <div class="fcs-today"></div>
      </div>
    </div>
    <div class="fcs-set">
      <div class="fcs-row"><span>Odak</span><div class="seg" data-k="work"></div></div>
      <div class="fcs-row"><span>Kısa mola</span><div class="seg" data-k="short"></div></div>
      <div class="fcs-row"><span>Uzun mola</span><div class="seg" data-k="long"></div></div>
      <div class="fcs-row"><span>Uzun moladan önce</span><div class="seg" data-k="rounds"></div></div>
      <div class="fcs-row"><span>Sıradakini kendiliğinden başlat</span><label class="qs-sw"><input type="checkbox" data-k="auto"><span></span></label></div>
      <div class="fcs-row"><span>Bitince ses</span><label class="qs-sw"><input type="checkbox" data-k="sound"><span></span></label></div>
    </div>`;
  panel.append(box);
  const OPTS = { work: [15, 25, 30, 45, 50, 60], short: [3, 5, 10], long: [10, 15, 20, 30], rounds: [2, 3, 4, 6] };
  const cfgF = () => ({ work: 25, short: 5, long: 15, rounds: 4, auto: true, sound: true, ...(pcfg?.focus || {}) });
  const drawSet = () => {
    const f = cfgF();
    box.querySelectorAll(".seg").forEach((sg) => {
      const k = sg.dataset.k;
      const unit = k === "rounds" ? "tur" : "dk";
      const setV = (v) => { pcfg.focus = { ...cfgF(), [k]: v }; drawSet(); invoke("set_focus_cfg", { focus: pcfg.focus }).catch(() => {}); };
      const chips = OPTS[k].map((v) => {
        const b = el("button", v === f[k] ? "on" : "", `${v} ${unit}`);
        b.onclick = () => setV(v);
        return b;
      });
      // Özel değer: listede yoksa vurgulu gösterilir; tıklayınca sayı yazılır
      const custom = !OPTS[k].includes(f[k]);
      const cb = el("button", "cust" + (custom ? " on" : ""), custom ? `${f[k]} ${unit}` : "Özel");
      cb.title = k === "rounds" ? "Tur sayısını yaz (1–12)" : "Dakikayı yaz (1–180)";
      cb.onclick = () => {
        const inp = el("input", "seg-in");
        inp.type = "number";
        inp.min = 1;
        inp.max = k === "rounds" ? 12 : 180;
        inp.value = f[k];
        inp.setAttribute("aria-label", cb.title);
        cb.replaceWith(inp);
        inp.focus();
        inp.select();
        let done = false;
        const commit = () => {
          if (done) return;
          done = true;
          const v = Math.round(Number(inp.value));
          if (v >= 1 && v <= Number(inp.max)) setV(v);
          else drawSet();
        };
        inp.onkeydown = (e) => { if (e.key === "Enter") commit(); if (e.key === "Escape") { done = true; drawSet(); } };
        inp.onblur = commit;
      };
      sg.replaceChildren(...chips, cb);
    });
    box.querySelectorAll("input[data-k]").forEach((i) => {
      i.checked = !!f[i.dataset.k];
      i.onchange = () => { pcfg.focus = { ...cfgF(), [i.dataset.k]: i.checked }; invoke("set_focus_cfg", { focus: pcfg.focus }).catch(() => {}); };
    });
  };
  drawSet();
  let st = await invoke("focus_state").catch(() => null);
  const val = box.querySelector(".val");
  const tick = () => {
    if (!st) return;
    const left = st.running ? st.ends_at - Date.now() : st.left_ms;
    const pct = st.total_ms ? Math.max(0, Math.min(1, left / st.total_ms)) : 1;
    val.style.strokeDashoffset = C * (1 - pct);
    const s = Math.max(0, Math.ceil(left / 1000));
    box.querySelector(".fcs-time").textContent = `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  };
  const draw = () => {
    if (!st) return;
    box.dataset.phase = st.phase;
    box.classList.toggle("running", st.running);
    box.querySelector(".fcs-phase").textContent = st.phase !== "idle" && !st.running ? "Duraklatıldı" : NAMES[st.phase];
    box.querySelector(".fcs-main").innerHTML = st.running
      ? `<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><rect x="6.5" y="5" width="4" height="14" rx="1.2"/><rect x="13.5" y="5" width="4" height="14" rx="1.2"/></svg> Duraklat`
      : `<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5.5v13l11-6.5z"/></svg> ${st.phase === "idle" ? "Başlat" : "Devam et"}`;
    const n = st.rounds || 4;
    const cur = st.phase === "focus" ? st.round : -1;
    box.querySelector(".fcs-dots").innerHTML = Array.from({ length: n }, (_, i) =>
      `<i class="${i < st.round ? "done" : ""}${i === cur ? " cur" : ""}"></i>`).join("") + `<span>${st.round}/${n} tur</span>`;
    box.querySelector(".fcs-today").textContent = st.today ? `Bugün ${st.today} odak turu · ${st.today_min} dk` : "Bugün henüz odak turu yok";
    tick();
  };
  draw();
  box.querySelector(".fcs-main").onclick = () => invoke("focus_cmd", { action: "toggle" }).catch(() => {});
  box.querySelector(".skip").onclick = () => invoke("focus_cmd", { action: "skip" }).catch(() => {});
  box.querySelector(".reset").onclick = () => invoke("focus_cmd", { action: "reset" }).catch(() => {});
  liveTimer = setInterval(tick, 1000);
  focusUnlisten?.();
  focusUnlisten = await listen("focus-state", (e) => { if (spec?.kind === "focus") { st = e.payload; draw(); } });
  relayout();
}

// ---------------------------------------------------------------------------
// Dünya saatleri
// ---------------------------------------------------------------------------
const CITIES = [
  ["Londra", "Europe/London"], ["Paris", "Europe/Paris"], ["Berlin", "Europe/Berlin"], ["Amsterdam", "Europe/Amsterdam"], ["Roma", "Europe/Rome"],
  ["Madrid", "Europe/Madrid"], ["Atina", "Europe/Athens"], ["Kiev", "Europe/Kyiv"], ["Moskova", "Europe/Moscow"], ["Dublin", "Europe/Dublin"],
  ["Lizbon", "Europe/Lisbon"], ["Varşova", "Europe/Warsaw"], ["Stockholm", "Europe/Stockholm"], ["Zürih", "Europe/Zurich"], ["Bakü", "Asia/Baku"],
  ["Tiflis", "Asia/Tbilisi"], ["Kahire", "Africa/Cairo"], ["Lagos", "Africa/Lagos"], ["Johannesburg", "Africa/Johannesburg"], ["Nairobi", "Africa/Nairobi"],
  ["Dubai", "Asia/Dubai"], ["Riyad", "Asia/Riyadh"], ["Doha", "Asia/Qatar"], ["Tahran", "Asia/Tehran"], ["Karaçi", "Asia/Karachi"],
  ["Mumbai", "Asia/Kolkata"], ["Yeni Delhi", "Asia/Kolkata"], ["Dakka", "Asia/Dhaka"], ["Bangkok", "Asia/Bangkok"], ["Singapur", "Asia/Singapore"],
  ["Hong Kong", "Asia/Hong_Kong"], ["Şanghay", "Asia/Shanghai"], ["Pekin", "Asia/Shanghai"], ["Seul", "Asia/Seoul"], ["Tokyo", "Asia/Tokyo"],
  ["Manila", "Asia/Manila"], ["Cakarta", "Asia/Jakarta"], ["Sidney", "Australia/Sydney"], ["Melbourne", "Australia/Melbourne"], ["Auckland", "Pacific/Auckland"],
  ["New York", "America/New_York"], ["Toronto", "America/Toronto"], ["Chicago", "America/Chicago"], ["Denver", "America/Denver"], ["Los Angeles", "America/Los_Angeles"],
  ["San Francisco", "America/Los_Angeles"], ["Meksiko", "America/Mexico_City"], ["Bogota", "America/Bogota"], ["Lima", "America/Lima"], ["São Paulo", "America/Sao_Paulo"],
  ["Buenos Aires", "America/Argentina/Buenos_Aires"], ["Honolulu", "Pacific/Honolulu"], ["Anchorage", "America/Anchorage"], ["İstanbul", "Europe/Istanbul"], ["Lefkoşa", "Asia/Nicosia"],
];
function wcOffset(tz) {
  try {
    const d = new Date();
    d.setSeconds(0, 0);
    const p = Object.fromEntries(new Intl.DateTimeFormat("en-US", { timeZone: tz, hourCycle: "h23", year: "numeric", month: "numeric", day: "numeric", hour: "numeric", minute: "numeric" }).formatToParts(d).map((x) => [x.type, x.value]));
    return Math.round((Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour, +p.minute) - d.getTime()) / 60000) + d.getTimezoneOffset();
  } catch { return 0; }
}
function renderWorld() {
  const box = el("div", "wcs");
  box.innerHTML = `
    <div class="clp-head"><strong>Dünya saatleri</strong><span class="clp-n"></span></div>
    <div class="wcs-list"></div>
    <form class="wcs-add"><span class="tdo-plus">${PI.plus}</span><input placeholder="Şehir ekle (ör. Tokyo)" aria-label="Şehir ekle" list="wcs-cities"><datalist id="wcs-cities">${CITIES.map(([n]) => `<option value="${esc(n)}">`).join("")}</datalist></form>
    <div class="wcs-tl"></div>`;
  panel.append(box);
  const listEl = box.querySelector(".wcs-list");
  const clocks = () => pcfg?.world_clocks || [];
  const save = (l) => { pcfg.world_clocks = l; invoke("set_world_clocks", { clocks: l }).catch(() => {}); draw(); };
  function draw() {
    const now = new Date();
    const localDay = now.toLocaleDateString("tr-TR", { day: "numeric" });
    const l = clocks();
    box.querySelector(".clp-n").textContent = l.length ? `${l.length} şehir` : "";
    listEl.replaceChildren(...l.map((w, i) => {
      const t = now.toLocaleTimeString("tr-TR", { timeZone: w.tz, hour: "2-digit", minute: "2-digit" });
      const h = +t.slice(0, 2), m = +t.slice(3, 5);
      const day = now.toLocaleDateString("tr-TR", { timeZone: w.tz, day: "numeric" });
      const off = wcOffset(w.tz);
      const offTxt = off ? `${off > 0 ? "+" : "−"}${Math.floor(Math.abs(off) / 60)}${Math.abs(off) % 60 ? ":" + String(Math.abs(off) % 60).padStart(2, "0") : ""} sa` : "aynı saat";
      const dayTxt = day === localDay ? "Bugün" : off > 0 ? "Yarın" : "Dün";
      const night = h < 7 || h >= 19;
      const r = el("div", "wcs-row" + (night ? " night" : ""));
      r.style.setProperty("--i", i);
      r.innerHTML = `<span class="wcs-face" style="--h:${(h % 12) * 30 + m / 2}deg;--m:${m * 6}deg"><i class="h"></i><i class="m"></i></span>
        <span class="wcs-tx"><b>${esc(w.name)}</b><small>${dayTxt} · ${offTxt} · ${night ? "gece" : "gündüz"}</small></span>
        <b class="wcs-t">${t}</b>
        <span class="wcs-acts"><button class="up" title="Yukarı" ${i ? "" : "disabled"}><svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M6 15l6-6 6 6"/></svg></button><button class="rm" title="Kaldır">${SVG.x}</button></span>`;
      r.querySelector(".rm").onclick = () => { r.classList.add("bye"); setTimeout(() => save(clocks().filter((_, j) => j !== i)), 200); };
      r.querySelector(".up").onclick = () => { const c = [...clocks()]; [c[i - 1], c[i]] = [c[i], c[i - 1]]; save(c); };
      return r;
    }));
    if (!l.length) listEl.innerHTML = `<div class="clp-empty">Şehir ekle; dock'taki saat aralarında döner.</div>`;
    // 24 saatlik zaman çizelgesi: her şehrin şu anki saati bir şeritte
    const tl = box.querySelector(".wcs-tl");
    tl.innerHTML = `<div class="tl-track">${[0, 6, 12, 18].map((h) => `<span style="left:${(h / 24) * 100}%">${String(h).padStart(2, "0")}</span>`).join("")}${l.map((w) => {
      const t = now.toLocaleTimeString("tr-TR", { timeZone: w.tz, hour: "2-digit", minute: "2-digit" });
      const pos = ((+t.slice(0, 2) * 60 + +t.slice(3, 5)) / 1440) * 100;
      return `<i style="left:${pos}%" title="${esc(w.name)} ${t}"><em>${esc(w.name.slice(0, 3))}</em></i>`;
    }).join("")}</div>`;
    tl.style.display = l.length ? "" : "none";
    relayout();
  }
  box.querySelector("form").onsubmit = (e) => {
    e.preventDefault();
    const inp = box.querySelector("input");
    const q = inp.value.trim().toLocaleLowerCase("tr-TR");
    const hit = CITIES.find(([n]) => n.toLocaleLowerCase("tr-TR") === q) || CITIES.find(([n]) => n.toLocaleLowerCase("tr-TR").startsWith(q));
    if (!hit) { inp.classList.add("bad"); setTimeout(() => inp.classList.remove("bad"), 500); return; }
    inp.value = "";
    if (clocks().length >= 8 || clocks().some((c) => c.name === hit[0])) return;
    save([...clocks(), { name: hit[0], tz: hit[1] }]);
  };
  draw();
  liveTimer = setInterval(draw, 30_000);
}

// ---------------------------------------------------------------------------
// Ağ hızı
// ---------------------------------------------------------------------------
function chartSvg(series, w, h) {
  const all = series.flatMap((x) => x.vals);
  const mx = Math.max(1, ...all);
  const step = w / Math.max(1, (series[0]?.vals.length || 2) - 1);
  return `<svg width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" class="nchart">${[0.25, 0.5, 0.75].map((f) => `<line x1="0" x2="${w}" y1="${h * f}" y2="${h * f}"/>`).join("")}${series.map((sr) => {
    const pts = sr.vals.map((v, i) => `${(i * step).toFixed(1)},${(h - 2 - (v / mx) * (h - 6)).toFixed(1)}`);
    return `<path d="M${pts.join("L")}L${w},${h}L0,${h}Z" fill="${sr.color}" opacity=".14"/><path d="M${pts.join("L")}" fill="none" stroke="${sr.color}" stroke-width="2" stroke-linejoin="round"/>`;
  }).join("")}</svg>`;
}
function renderNet() {
  const box = el("div", "nets");
  box.innerHTML = `
    <div class="clp-head"><strong>Ağ hızı</strong><span class="clp-n">son 2 dakika</span></div>
    <div class="nets-now">
      <div class="nn dn"><small>İndirme</small><b>–</b><span></span></div>
      <div class="nn up"><small>Yükleme</small><b>–</b><span></span></div>
    </div>
    <div class="nets-chart"></div>
    <div class="clp-foot">Tüm fiziksel ağ bağdaştırıcılarının toplamı. Panel açıkken saniyede bir ölçülür.</div>`;
  panel.append(box);
  const rx = [], tx = [];
  let last = null, tot = [0, 0];
  const tick = async () => {
    const [r, t] = (await invoke("net_counters").catch(() => [0, 0])) || [0, 0];
    const now = Date.now();
    if (last && r >= last.r) {
      const dt = (now - last.at) / 1000 || 1;
      rx.push((r - last.r) / dt); tx.push((t - last.t) / dt);
      tot[0] += r - last.r; tot[1] += t - last.t;
      if (rx.length > 120) { rx.shift(); tx.shift(); }
    }
    last = { r, t, at: now };
    const put = (cls, arr, total) => {
      const n = box.querySelector(`.nn.${cls}`);
      n.querySelector("b").textContent = fmtRate(arr.at(-1) || 0);
      n.querySelector("span").textContent = `Tepe ${fmtRate(Math.max(0, ...arr))} · bu panelde ${fmtBytes(total)}`;
    };
    put("dn", rx, tot[0]); put("up", tx, tot[1]);
    const vals = (a) => (a.length > 1 ? a : [0, 0]);
    box.querySelector(".nets-chart").innerHTML = chartSvg([{ vals: vals(rx), color: "#38BDF8" }, { vals: vals(tx), color: "#A78BFA" }], 340, 120);
  };
  tick();
  liveTimer = setInterval(tick, 1000);
  relayout();
}

// ---------------------------------------------------------------------------
// Disk ve çöp kutusu
// ---------------------------------------------------------------------------
async function renderDisk() {
  const box = el("div", "dsk");
  box.innerHTML = `<div class="clp-head"><strong>Diskler</strong><span class="clp-n"></span></div><div class="dsk-list"></div>
    <div class="dsk-bin"><span class="bin-ic"><svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/></svg></span>
      <span class="bin-tx"><b>Geri Dönüşüm Kutusu</b><small></small></span>
      <button class="bin-open">Aç</button><button class="bin-empty">Boşalt</button></div>`;
  panel.append(box);
  const draw = async () => {
    const d = await invoke("disk_info").catch(() => null);
    if (!d) return;
    box.querySelector(".clp-n").textContent = `${d.drives.length} sürücü`;
    box.querySelector(".dsk-list").replaceChildren(...d.drives.map((x, i) => {
      const used = 1 - x.free / x.total;
      const r = el("div", "dsk-row" + (used > 0.9 ? " warn" : ""));
      r.style.setProperty("--i", i);
      r.innerHTML = `<span class="dsk-letter">${esc(x.root.slice(0, 2))}</span>
        <span class="dsk-main"><span class="dsk-top"><b>${esc(x.label || (x.kind === "removable" ? "Çıkarılabilir disk" : x.kind === "network" ? "Ağ sürücüsü" : "Yerel disk"))}</b><small>${fmtBytes(x.free)} boş / ${fmtBytes(x.total)}</small></span>
        <span class="dsk-bar"><i style="width:${(used * 100).toFixed(1)}%"></i></span></span>`;
      r.onclick = () => invoke("launch_app", { path: x.root, args: null }).catch(() => {});
      r.title = `${x.root} klasörünü aç`;
      return r;
    }));
    box.querySelector(".bin-tx small").textContent = d.bin_items ? `${d.bin_items.toLocaleString("tr-TR")} öğe · ${fmtBytes(d.bin_bytes)}` : "Boş";
    box.querySelector(".bin-empty").disabled = !d.bin_items;
    relayout();
  };
  box.querySelector(".bin-open").onclick = () => invoke("launch_app", { path: "shell:RecycleBinFolder", args: null }).catch(() => {});
  const empty = box.querySelector(".bin-empty");
  empty.onclick = async () => {
    if (!empty.classList.contains("arm")) {
      empty.classList.add("arm");
      empty.textContent = "Emin misin?";
      setTimeout(() => { empty.classList.remove("arm"); empty.textContent = "Boşalt"; }, 3000);
      return;
    }
    empty.classList.remove("arm");
    empty.textContent = "Boşaltılıyor…";
    await invoke("recycle_empty").catch(() => {});
    empty.textContent = "Boşalt";
    box.querySelector(".dsk-bin").classList.add("flash");
    TAURI?.event.emit("disk-changed");
    draw();
  };
  await draw();
}

// ---------------------------------------------------------------------------
// Kripto
// ---------------------------------------------------------------------------
async function renderCrypto() {
  const box = el("div", "cry");
  const cur = () => pcfg?.crypto?.currency || "try";
  box.innerHTML = `<div class="clp-head"><strong>Kripto</strong><span class="clp-n">7 günlük</span>
      <div class="seg cur">${["try", "usd", "eur"].map((c) => `<button data-c="${c}">${c.toUpperCase()}</button>`).join("")}</div></div>
    <div class="cry-list"></div>
    <div class="cry-add"></div>
    <div class="clp-foot">Kaynak: CoinGecko · 2 dakikada bir yenilenir. Yatırım tavsiyesi değildir.</div>`;
  panel.append(box);
  const save = (c) => { pcfg.crypto = c; invoke("set_cfg_value", { key: "crypto", value: c }).catch(() => {}); draw(true); };
  async function draw(force) {
    const c = pcfg?.crypto || { coins: ["bitcoin", "ethereum", "solana"], currency: "try" };
    box.querySelectorAll(".seg.cur button").forEach((b) => { b.classList.toggle("on", b.dataset.c === c.currency); b.onclick = () => save({ ...c, currency: b.dataset.c }); });
    let data = [];
    try { data = await Crypto.get(pcfg, force); } catch { box.querySelector(".cry-list").innerHTML = `<div class="clp-empty">CoinGecko'ya ulaşılamadı.</div>`; }
    if (data.length) box.querySelector(".cry-list").replaceChildren(...data.map((x, i) => {
      const r = el("div", "cry-row");
      r.style.setProperty("--i", i);
      const up = x.change >= 0;
      r.innerHTML = `<span class="cr-ic">${x.image ? `<img src="${x.image}" alt="">` : `<b>${esc(x.sym[0])}</b>`}</span>
        <span class="cry-tx"><b>${esc(x.name)}</b><small>${esc(x.sym)}</small></span>
        <span class="cry-sp" style="--sc:${up ? "#22C55E" : "#EF4444"}">${spark(x.spark, 90, 30)}</span>
        <span class="cry-pr"><b>${Crypto.fmt(x.price, x.cur)}</b><small class="${up ? "up" : "down"}">${up ? "▲" : "▼"} %${fmt2(Math.abs(x.change))}</small></span>
        <button class="cry-x" title="Kaldır">${SVG.x}</button>`;
      r.querySelector(".cry-x").onclick = () => save({ ...c, coins: c.coins.filter((id) => id !== x.id) });
      return r;
    }));
    const add = box.querySelector(".cry-add");
    add.replaceChildren(...Object.entries(COINS).filter(([id]) => !c.coins.includes(id)).slice(0, 12).map(([id, [sym]]) => {
      const b = el("button", "chip", `+ ${sym}`);
      b.title = COINS[id][1];
      b.onclick = () => save({ ...c, coins: [...c.coins, id].slice(0, 8) });
      return b;
    }));
    relayout();
  }
  function spark(vals, w, h) {
    if (!vals?.length) return "";
    const mx = Math.max(...vals), mn = Math.min(...vals), rg = mx - mn || 1, st = w / (vals.length - 1);
    const pts = vals.map((v, i) => `${(i * st).toFixed(1)},${(h - 1 - ((v - mn) / rg) * (h - 2)).toFixed(1)}`);
    return `<svg width="${w}" height="${h}" class="mini-spark"><path d="M${pts.join("L")}L${w},${h}L0,${h}Z" class="a"/><path d="M${pts.join("L")}" class="l"/></svg>`;
  }
  await draw();
}

// ---------------------------------------------------------------------------
// Namaz vakitleri
// ---------------------------------------------------------------------------
async function renderPrayer() {
  const box = el("div", "pry");
  box.innerHTML = `<div class="clp-head"><strong>Namaz vakitleri</strong><span class="clp-n"></span></div>
    <div class="pry-next"><small></small><b></b><span class="pry-bar"><i></i></span></div>
    <div class="pry-list"></div>
    <div class="clp-foot">Diyanet hesaplama yöntemi (Aladhan). Konum, hava durumu ayarındaki şehirden alınır.</div>`;
  panel.append(box);
  let st = null;
  const draw = () => {
    if (!st) return;
    const now = Date.now();
    box.querySelector(".clp-n").textContent = `${st.city} · ${new Date().toLocaleDateString("tr-TR", { day: "numeric", month: "long", weekday: "long" })}`;
    box.querySelector(".pry-next small").textContent = `${st.next.name} vaktine`;
    box.querySelector(".pry-next b").textContent = fmtLeft(st.next.at - now);
    const pct = Math.min(1, Math.max(0, (now - st.prev.at) / (st.next.at - st.prev.at)));
    box.querySelector(".pry-bar i").style.width = `${pct * 100}%`;
    box.querySelector(".pry-list").replaceChildren(...st.today.map((p, i) => {
      const r = el("div", "pry-row" + (p.key === st.next.key && p.at === st.next.at ? " next" : "") + (p.at <= now ? " past" : ""));
      r.style.setProperty("--i", i);
      r.innerHTML = `<b>${p.name}</b><span>${p.hm}</span>`;
      return r;
    }));
    relayout();
  };
  try { st = await Prayer.get(pcfg); draw(); }
  catch { box.querySelector(".pry-list").innerHTML = `<div class="clp-empty">Vakitler alınamadı. İnternet bağlantını kontrol et.</div>`; relayout(); }
  liveTimer = setInterval(async () => { if (st && st.next.at <= Date.now()) st = await Prayer.get(pcfg).catch(() => st); draw(); }, 30_000);
}

// ---------------------------------------------------------------------------
// Geri sayım
// ---------------------------------------------------------------------------
function renderCountdown() {
  const box = el("div", "cdn");
  box.innerHTML = `<div class="clp-head"><strong>Geri sayım</strong><span class="clp-n"></span></div>
    <div class="cdn-list"></div>
    <form class="cdn-add"><input class="nm" placeholder="Ne için? (ör. Tatil)" maxlength="40" aria-label="Ad"><input class="dt" type="date" aria-label="Tarih"><button>Ekle</button></form>`;
  panel.append(box);
  const list = () => pcfg?.countdowns || [];
  const save = (l) => { pcfg.countdowns = l; invoke("set_cfg_value", { key: "countdowns", value: l }).catch(() => {}); draw(); };
  function draw() {
    const now = Date.now();
    const items = list().map((x, idx) => ({ ...x, idx, at: new Date(x.date.length <= 10 ? x.date + "T00:00" : x.date).getTime() })).sort((a, b) => a.at - b.at);
    box.querySelector(".clp-n").textContent = items.length ? `${items.length} hedef` : "";
    if (!items.length) {
      const y = new Date().getFullYear() + 1;
      items.push({ name: "Yılbaşı (otomatik)", date: `${y}-01-01`, at: new Date(`${y}-01-01T00:00`).getTime(), idx: -1 });
    }
    box.querySelector(".cdn-list").replaceChildren(...items.map((x, i) => {
      const ms = x.at - now, d = Math.floor(Math.abs(ms) / 864e5), h = Math.floor((Math.abs(ms) % 864e5) / 36e5);
      const r = el("div", "cdn-row" + (ms <= 0 ? " done" : ""));
      r.style.setProperty("--i", i);
      r.innerHTML = `<span class="cd-box"><b>${d}</b><small>gün</small></span>
        <span class="cdn-tx"><b>${esc(x.name)}</b><small>${new Date(x.at).toLocaleDateString("tr-TR", { day: "numeric", month: "long", year: "numeric", weekday: "short" })} · ${ms <= 0 ? "geçti" : `${d} gün ${h} saat kaldı`}</small></span>
        ${x.idx >= 0 ? `<button class="cdn-x" title="Kaldır">${SVG.x}</button>` : ""}`;
      r.querySelector(".cdn-x")?.addEventListener("click", () => save(list().filter((_, j) => j !== x.idx)));
      return r;
    }));
    relayout();
  }
  const dt = box.querySelector(".dt");
  dt.valueAsDate = new Date(Date.now() + 7 * 864e5);
  box.querySelector("form").onsubmit = (e) => {
    e.preventDefault();
    const nm = box.querySelector(".nm").value.trim();
    if (!nm || !dt.value) { box.querySelector(nm ? ".dt" : ".nm").classList.add("bad"); setTimeout(() => box.querySelectorAll(".bad").forEach((x) => x.classList.remove("bad")), 500); return; }
    box.querySelector(".nm").value = "";
    save([...list(), { name: nm, date: dt.value }].slice(0, 12));
  };
  draw();
  liveTimer = setInterval(draw, 60_000);
}

// ---------------------------------------------------------------------------
// Hesap makinesi (kendi ayrıştırıcısı; eval yok)
// ---------------------------------------------------------------------------
function calcEval(src) {
  const s = src.replace(/×/g, "*").replace(/÷/g, "/").replace(/,/g, ".").replace(/\s+/g, "");
  let i = 0;
  const peek = () => s[i];
  const num = () => {
    const m = /^\d*\.?\d+(e[+-]?\d+)?/i.exec(s.slice(i));
    if (!m) throw new Error("sayı bekleniyor");
    i += m[0].length;
    return parseFloat(m[0]);
  };
  const factor = () => {
    if (peek() === "-") { i++; return -factor(); }
    if (peek() === "+") { i++; return factor(); }
    let v;
    if (peek() === "(") { i++; v = expr(); if (peek() !== ")") throw new Error(")"); i++; }
    else if (s.startsWith("√", i)) { i++; v = Math.sqrt(factor()); }
    else if (s.startsWith("π", i)) { i++; v = Math.PI; }
    else v = num();
    while (peek() === "%" ) { i++; v = v / 100; }
    if (peek() === "^") { i++; v = v ** factor(); }
    return v;
  };
  const term = () => {
    let v = factor();
    while (peek() === "*" || peek() === "/") { const o = s[i++]; const r = factor(); v = o === "*" ? v * r : v / r; }
    return v;
  };
  const expr = () => {
    let v = term();
    while (peek() === "+" || peek() === "-") { const o = s[i++]; const r = term(); v = o === "+" ? v + r : v - r; }
    return v;
  };
  const v = expr();
  if (i < s.length) throw new Error("beklenmeyen " + s[i]);
  return v;
}
const calcFmt = (v) => (Number.isFinite(v) ? v.toLocaleString("tr-TR", { maximumFractionDigits: 10 }) : "Tanımsız");
let calcHist = [];
function renderCalc() {
  const box = el("div", "clc");
  box.innerHTML = `<div class="clc-hist"></div>
    <input class="clc-in" placeholder="0" aria-label="İşlem" inputmode="decimal" spellcheck="false">
    <div class="clc-res"></div>
    <div class="clc-keys">${["C", "(", ")", "÷", "7", "8", "9", "×", "4", "5", "6", "-", "1", "2", "3", "+", "%", "0", ",", "="].map((k) => `<button data-k="${k}" class="${/[÷×\-+=]/.test(k) ? "op" : k === "C" ? "fn" : ""}${k === "=" ? " eq" : ""}">${k}</button>`).join("")}</div>`;
  panel.append(box);
  const inp = box.querySelector(".clc-in"), res = box.querySelector(".clc-res");
  const live = () => {
    try { res.textContent = inp.value.trim() ? "= " + calcFmt(calcEval(inp.value)) : ""; res.classList.remove("err"); }
    catch { res.textContent = ""; }
  };
  const drawHist = () => {
    box.querySelector(".clc-hist").replaceChildren(...calcHist.slice(-4).map((h) => {
      const r = el("button", "clc-h", `<span>${esc(h.e)}</span><b>${esc(h.r)}</b>`);
      r.title = "Tıkla: sonucu kopyala";
      r.onclick = () => { invoke("copy_text", { text: h.r }).catch(() => {}); r.classList.add("copied"); setTimeout(() => r.classList.remove("copied"), 700); };
      return r;
    }));
    relayout();
  };
  const equals = () => {
    if (!inp.value.trim()) return;
    try {
      const v = calcEval(inp.value);
      const r = calcFmt(v);
      calcHist.push({ e: inp.value, r });
      calcHist = calcHist.slice(-20);
      TAURI?.event.emit("calc-result", r);
      inp.value = Number.isFinite(v) ? String(v).replace(".", ",") : "";
      res.textContent = "";
      box.classList.remove("shake"); void box.offsetWidth;
      drawHist();
    } catch { res.textContent = "Hatalı işlem"; res.classList.add("err"); }
  };
  box.querySelectorAll(".clc-keys button").forEach((b) => b.onclick = () => {
    const k = b.dataset.k;
    if (k === "C") inp.value = "";
    else if (k === "=") return equals();
    else inp.value += k;
    inp.focus();
    live();
  });
  inp.oninput = live;
  inp.onkeydown = (e) => {
    if (e.key === "Enter" || e.key === "=") { e.preventDefault(); equals(); }
    if (e.key === "Escape") { inp.value = ""; live(); }
  };
  drawHist();
  setTimeout(() => inp.focus(), 60);
}

// ---------------------------------------------------------------------------
// Renk seçici
// ---------------------------------------------------------------------------
const hexToRgb = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
function rgbToHsl([r, g, b]) {
  r /= 255; g /= 255; b /= 255;
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b), l = (mx + mn) / 2;
  if (mx === mn) return [0, 0, Math.round(l * 100)];
  const d = mx - mn, s = l > 0.5 ? d / (2 - mx - mn) : d / (mx + mn);
  const h = mx === r ? (g - b) / d + (g < b ? 6 : 0) : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [Math.round(h * 60), Math.round(s * 100), Math.round(l * 100)];
}
async function renderColor() {
  const box = el("div", "clr");
  box.innerHTML = `<div class="clp-head"><strong>Renk seçici</strong><button class="clr-clear">Geçmişi sil</button></div>
    <div class="clr-main"><span class="clr-big"></span><div class="clr-fmts"></div></div>
    <button class="clr-pick">${'<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.5 4.5l5 5M8 11l5 5M6.5 12.5l6-6 5 5-6 6H6.5z"/><path d="M6.5 17.5L4 20"/></svg>'} Ekrandan renk al</button>
    <div class="clr-hist"></div>
    <div class="clp-foot">Sol tık seçer (tıklama alttaki uygulamaya gitmez), sağ tık ya da Esc iptal eder. Seçilen renk HEX olarak panoya kopyalanır.</div>`;
  panel.append(box);
  let hist = (await invoke("color_history").catch(() => [])) || [];
  let sel = hist[0] || null;
  const copy = (t, b) => { invoke("copy_text", { text: t }).catch(() => {}); b.classList.add("copied"); setTimeout(() => b.classList.remove("copied"), 700); };
  const draw = () => {
    box.classList.toggle("empty", !sel);
    box.querySelector(".clr-big").style.background = sel || "transparent";
    if (sel) {
      const rgb = hexToRgb(sel), hsl = rgbToHsl(rgb);
      box.querySelector(".clr-fmts").replaceChildren(...[["HEX", sel], ["RGB", `rgb(${rgb.join(", ")})`], ["HSL", `hsl(${hsl[0]}, ${hsl[1]}%, ${hsl[2]}%)`]].map(([k, v]) => {
        const b = el("button", "clr-f", `<small>${k}</small><b>${v}</b>`);
        b.title = "Kopyala";
        b.onclick = () => copy(v, b);
        return b;
      }));
    } else box.querySelector(".clr-fmts").innerHTML = `<div class="clp-empty" style="padding:6px">Henüz renk seçmedin.</div>`;
    box.querySelector(".clr-hist").replaceChildren(...hist.map((h) => {
      const b = el("button", "clr-sw" + (h === sel ? " on" : ""));
      b.style.background = h;
      b.title = h;
      b.onclick = () => { sel = h; draw(); };
      return b;
    }));
    box.querySelector(".clr-clear").style.visibility = hist.length ? "visible" : "hidden";
    relayout();
  };
  box.querySelector(".clr-pick").onclick = () => { invoke("popover_close").catch(() => {}); invoke("color_pick").catch(() => {}); };
  box.querySelector(".clr-clear").onclick = () => { invoke("color_forget").catch(() => {}); hist = []; sel = null; draw(); };
  draw();
}

// ---------------------------------------------------------------------------
// Hızlı çeviri
// ---------------------------------------------------------------------------
const LANGS = [["tr", "Türkçe"], ["en", "İngilizce"], ["de", "Almanca"], ["fr", "Fransızca"], ["es", "İspanyolca"], ["ar", "Arapça"], ["ru", "Rusça"], ["it", "İtalyanca"]];
let trTo = "tr";
async function renderTranslate() {
  const box = el("div", "trn");
  box.innerHTML = `<div class="clp-head"><strong>Çeviri</strong><span class="clp-n trn-det"></span>
      <select class="trn-to" aria-label="Hedef dil">${LANGS.map(([k, n]) => `<option value="${k}">${n}</option>`).join("")}</select></div>
    <textarea class="trn-in" placeholder="Çevrilecek metni yaz ya da yapıştır" spellcheck="false"></textarea>
    <div class="trn-btns"><button class="trn-clip">Panodakini çevir</button><button class="trn-swap" title="Dilleri değiştir">⇅</button><button class="trn-go">Çevir</button></div>
    <div class="trn-out"><span class="ph">Çeviri burada görünecek</span></div>
    <div class="trn-acts"><button class="trn-copy">Kopyala</button></div>`;
  panel.append(box);
  const inp = box.querySelector(".trn-in"), out = box.querySelector(".trn-out"), to = box.querySelector(".trn-to");
  to.value = trTo;
  let t = 0, lastOut = "";
  const go = async () => {
    const q = inp.value.trim();
    if (!q) { out.innerHTML = `<span class="ph">Çeviri burada görünecek</span>`; lastOut = ""; relayout(); return; }
    out.classList.add("busy");
    try {
      const r = await translateText(q.slice(0, 4000), to.value);
      lastOut = r.text;
      out.textContent = r.text;
      const from = LANGS.find(([k]) => k === r.from)?.[1] || r.from;
      box.querySelector(".trn-det").textContent = from ? `${from} → ${LANGS.find(([k]) => k === to.value)[1]}` : "";
    } catch { out.innerHTML = `<span class="ph">Çeviri servisine ulaşılamadı.</span>`; }
    out.classList.remove("busy");
    relayout();
  };
  inp.oninput = () => { clearTimeout(t); t = setTimeout(go, 500); };
  inp.onkeydown = (e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) { e.preventDefault(); go(); } };
  to.onchange = () => { trTo = to.value; go(); };
  box.querySelector(".trn-go").onclick = go;
  box.querySelector(".trn-swap").onclick = () => {
    if (lastOut) inp.value = lastOut;
    trTo = to.value = to.value === "tr" ? "en" : "tr";
    go();
  };
  box.querySelector(".trn-clip").onclick = async () => {
    const items = (await invoke("clip_list").catch(() => [])) || [];
    const txt = items.find((x) => x.kind === "text")?.text;
    if (txt) { inp.value = txt; go(); }
  };
  const cp = box.querySelector(".trn-copy");
  cp.onclick = () => { if (!lastOut) return; invoke("copy_text", { text: lastOut }).catch(() => {}); cp.textContent = "Kopyalandı ✓"; setTimeout(() => (cp.textContent = "Kopyala"), 900); };
  setTimeout(() => inp.focus(), 60);
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
  // Dock'taki müzik widget'ıyla aynı genişlik (panel dolgusu ve kenarlık düşülür)
  if (p?.width) box.style.width = `${Math.max(200, Math.round(p.width) - 34)}px`;
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
    // Sağ tık: Gezgin'deki menünün aynısı (Shift ile genişletilmiş menü)
    it.oncontextmenu = (ev) => {
      ev.preventDefault();
      invoke("shell_menu", { path: e.path, extended: ev.shiftKey });
    };
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
  if (cmd === "get_config") return { theme: q.get("theme") || "dark", backdrop: "acrylic", accent: "#2F7BFF", dynamic: { enabled: q.has("wall"), tint: 45 }, weather: { enabled: true, auto_location: true, city: "İstanbul", lat: 41, lon: 29 },
    focus: { work: 25, short: 5, long: 15, rounds: 4, auto: true, sound: true },
    crypto: { coins: ["bitcoin", "ethereum", "solana"], currency: "try" },
    countdowns: [{ name: "Tatil", date: "2026-12-24" }, { name: "Proje teslimi", date: "2026-12-05" }],
    world_clocks: [{ name: "Londra", tz: "Europe/London" }, { name: "Kahire", tz: "Africa/Cairo" }, { name: "New York", tz: "America/New_York" }, { name: "Tokyo", tz: "Asia/Tokyo" }] };
  if (cmd === "net_counters") { window.__nb = (window.__nb || 5e9) + 1.5e6 + Math.random() * 3e6; return [window.__nb, window.__nb / 9]; }
  if (cmd === "disk_info") return { drives: [{ root: "C:\\", label: "Windows", total: 1e12, free: 2.1e11, kind: "fixed" }, { root: "D:\\", label: "Veri", total: 2e12, free: 1.4e11, kind: "fixed" }, { root: "E:\\", label: "", total: 6.4e10, free: 3.2e10, kind: "removable" }], bin_items: 214, bin_bytes: 3.4e9 };
  if (cmd === "color_history") return ["#2F7BFF", "#F97316", "#22C55E", "#EC4899", "#0EA5E9", "#FACC15", "#111827"];
  if (cmd === "notes_list") return [
    { id: "a", text: "Cuma 15:00 haftalık toplantı\n- Sunumu bitir\n- Faturaları gönder", color: "yellow", desktop: true, updated: Date.now() },
    { id: "b", text: "Wi-Fi şifresi misafir: kahve2026", color: "pink", desktop: false, updated: Date.now() - 6e5 },
    { id: "c", text: "Kitap: Sapiens bitince Homo Deus", color: "blue", desktop: false, updated: Date.now() - 9e6 },
    { id: "d", text: "", color: "green", desktop: false, updated: Date.now() - 9e7 },
  ];
  if (cmd === "todos_list") return [
    { id: "1", text: "Rapor taslağını gözden geçir", done: false }, { id: "2", text: "Diş randevusu al", done: false },
    { id: "3", text: "Market: süt, ekmek, zeytin", done: false }, { id: "4", text: "Faturayı öde", done: true }, { id: "5", text: "Yedekleri kontrol et", done: true },
  ];
  if (cmd === "focus_state") return { phase: "focus", running: true, ends_at: Date.now() + 14 * 60e3 + 32e3, left_ms: 0, total_ms: 25 * 60e3, round: 1, rounds: 4, today: 3, today_min: 75 };
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
    if (q.get("kind") === "notes") return { kind: "notes", serial: 1, key: "notes", payload: null };
    if (q.get("kind") === "todo") return { kind: "todo", serial: 1, key: "todo", payload: null };
    if (q.get("kind") === "focus") return { kind: "focus", serial: 1, key: "focus", payload: null };
    if (q.get("kind") === "world") return { kind: "world", serial: 1, key: "world", payload: null };
    if (q.get("kind") === "net") return { kind: "net", serial: 1, key: "net", payload: null };
    if (q.get("kind") === "disk") return { kind: "disk", serial: 1, key: "disk", payload: null };
    if (q.get("kind") === "crypto") return { kind: "crypto", serial: 1, key: "crypto", payload: null };
    if (q.get("kind") === "prayer") return { kind: "prayer", serial: 1, key: "prayer", payload: null };
    if (q.get("kind") === "countdown") return { kind: "countdown", serial: 1, key: "countdown", payload: null };
    if (q.get("kind") === "calc") return { kind: "calc", serial: 1, key: "calc", payload: null };
    if (q.get("kind") === "color") return { kind: "color", serial: 1, key: "color", payload: null };
    if (q.get("kind") === "translate") return { kind: "translate", serial: 1, key: "translate", payload: null };
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
    ...Array.from({ length: q.has("many") ? 18 : 0 }, (_, i) => ({ id: 10 + i, kind: "text", text: `Örnek kopyalanan metin ${i + 1}: lorem ipsum dolor sit amet`, source: "Chrome", source_path: "", time: Date.now() - (i + 2) * 3600e3, pinned: false })),
  ];
  if (cmd === "claude_usage") return { state: q.get("cstate") || "ok", plan: "Max", updated: Date.now() - 120000, message: "", limits: q.get("cstate") === "no_claude_code" ? [] : [
    { label: "Oturum", percent: 34, resets_at: new Date(Date.now() + 2.3 * 3600e3).toISOString() },
    { label: "Haftalık", percent: 58, resets_at: new Date(Date.now() + 3.4 * 86400e3).toISOString() },
    { label: "Opus", percent: 87, resets_at: new Date(Date.now() + 3.4 * 86400e3).toISOString() },
  ] };
  if (cmd === "wallpaper") return MOCK.wallpaper(q.get("wall"));
  if (cmd === "fetch_text") return /open-meteo|geojs|coingecko|aladhan|translate\.googleapis/.test(args.url) ? MOCK.fetch(args.url) : mockFetch(args.url);
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
