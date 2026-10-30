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
const darkQuery = matchMedia("(prefers-color-scheme: dark)");
async function applyTheme() {
  const cfg = await invoke("get_config").catch(() => null);
  if (!cfg) return;
  const root = document.documentElement;
  const dark = cfg.theme === "dark" || (cfg.theme !== "light" && darkQuery.matches);
  root.dataset.theme = dark ? "dark" : "light";
  root.dataset.backdrop = cfg.backdrop;
  root.style.setProperty("--accent", cfg.accent || "#2F7BFF");
}

// ---------------------------------------------------------------------------
// Yerleşim
// ---------------------------------------------------------------------------
function relayout() {
  requestAnimationFrame(() => {
    const r = panel.getBoundingClientRect();
    invoke("popover_layout", { width: Math.ceil(r.width), height: Math.ceil(r.height), serial }).catch(console.error);
  });
}

async function show(s) {
  serial = s.serial;
  spec = s;
  panel.classList.remove("in");
  await applyTheme();
  panel.replaceChildren();
  if (s.kind === "calendar") renderCalendar();
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
  panel.classList.remove("in");
  setTimeout(() => invoke("popover_hidden").catch(() => {}), 200);
});
listen("config-changed", applyTheme);

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
// Başlangıç
// ---------------------------------------------------------------------------
(async () => {
  const pending = await invoke("popover_pending").catch(() => null);
  if (pending) show(pending);
})();

// Tarayıcıda önizleme (popover.html?kind=calendar | windows)
async function mockInvoke(cmd) {
  const q = new URLSearchParams(location.search);
  if (cmd === "get_config") return { theme: q.get("theme") || "dark", backdrop: "acrylic", accent: "#2F7BFF" };
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
    return { kind: "calendar", serial: 1, key: "cal", payload: null };
  }
  return null;
}
