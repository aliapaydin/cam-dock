// Masaüstündeki yapışkan not penceresi.
"use strict";

const TAURI = window.__TAURI__;
const invoke = TAURI ? TAURI.core.invoke : mockInvoke;
const listen = TAURI ? TAURI.event.listen : async () => () => {};
const $ = (s) => document.querySelector(s);

const COLORS = [["yellow", "#FFE36E"], ["pink", "#FF9EC8"], ["blue", "#8EC5FF"], ["green", "#8EE39A"], ["purple", "#B79BFF"], ["orange", "#FFB070"], ["gray", "#2C313D"]];
const ICON = {
  top: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 4h6l-1 6 4 3v2H6v-2l4-3z"/><path d="M12 15v6"/></svg>',
  hide: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>',
};

const paper = $("#paper");
const text = $("#text");
let note = null;
let saveT = 0;

function when(ms) {
  if (!ms) return "";
  const d = new Date(ms);
  const today = new Date().toDateString() === d.toDateString();
  return today
    ? `Bugün ${d.toLocaleTimeString("tr-TR", { hour: "2-digit", minute: "2-digit" })}`
    : d.toLocaleDateString("tr-TR", { day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" });
}

function paint() {
  document.documentElement.dataset.color = note.color || "yellow";
  $("#when").textContent = when(note.updated);
  $("#top").classList.toggle("on", !!note.top);
  $("#top").title = note.top ? "Üstte tutma" : "Diğer pencerelerin üstünde tut";
  for (const b of $("#colors").children) b.classList.toggle("on", b.dataset.c === note.color);
}

function build() {
  $("#colors").replaceChildren(...COLORS.map(([c, hex]) => {
    const b = document.createElement("button");
    b.dataset.c = c;
    b.style.background = hex;
    b.setAttribute("aria-label", c);
    b.onclick = () => { note.color = c; paint(); invoke("note_update", { id: note.id, color: c }).catch(() => {}); };
    return b;
  }));
  $("#top").innerHTML = ICON.top;
  $("#hide").innerHTML = ICON.hide;
  $("#top").onclick = () => { note.top = !note.top; paint(); invoke("note_update", { id: note.id, top: note.top }).catch(() => {}); };
  $("#hide").onclick = () => invoke("note_desktop", { id: note.id, on: false }).catch(() => {});
  text.value = note.text;
  text.oninput = () => {
    clearTimeout(saveT);
    saveT = setTimeout(() => invoke("note_update", { id: note.id, text: text.value }).catch(() => {}), 350);
  };
  paint();
}

// Sürükleme: kâğıdın üst şeridinden. Pencereyi Rust taşır; bırakınca kenara/köşeye yaslanır.
let drag = null;
$("#bar").addEventListener("pointerdown", (e) => {
  if (e.button !== 0 || e.target.closest("button")) return;
  drag = { x: e.screenX, y: e.screenY, id: e.pointerId, raf: 0 };
  $("#bar").setPointerCapture(e.pointerId);
  paper.classList.add("lift");
  invoke("note_drag_start").catch(() => {});
});
$("#bar").addEventListener("pointermove", (e) => {
  if (!drag || e.pointerId !== drag.id) return;
  const r = devicePixelRatio || 1;
  const dx = Math.round((e.screenX - drag.x) * r), dy = Math.round((e.screenY - drag.y) * r);
  if (drag.raf) return;
  drag.raf = requestAnimationFrame(() => { if (drag) drag.raf = 0; invoke("note_drag", { dx, dy }).catch(() => {}); });
});
const endDrag = (e) => {
  if (!drag || e.pointerId !== drag.id) return;
  drag = null;
  paper.classList.remove("lift");
  invoke("note_drag_end").catch(() => {});
};
$("#bar").addEventListener("pointerup", endDrag);
$("#bar").addEventListener("pointercancel", endDrag);

// Boyutlandırma: sağ alt kıvrık köşe
let rs = null;
$("#fold").addEventListener("pointerdown", (e) => {
  if (e.button !== 0) return;
  e.preventDefault();
  const r = devicePixelRatio || 1;
  rs = { x: e.screenX, y: e.screenY, w: innerWidth * r, h: innerHeight * r, id: e.pointerId, raf: 0 };
  $("#fold").setPointerCapture(e.pointerId);
});
$("#fold").addEventListener("pointermove", (e) => {
  if (!rs || e.pointerId !== rs.id || rs.raf) return;
  const r = devicePixelRatio || 1;
  const w = Math.round(rs.w + (e.screenX - rs.x) * r), h = Math.round(rs.h + (e.screenY - rs.y) * r);
  rs.raf = requestAnimationFrame(() => { if (rs) rs.raf = 0; invoke("note_resize", { w, h, done: false }).catch(() => {}); });
});
$("#fold").addEventListener("pointerup", (e) => {
  if (!rs) return;
  const r = devicePixelRatio || 1;
  const w = Math.round(rs.w + (e.screenX - rs.x) * r), h = Math.round(rs.h + (e.screenY - rs.y) * r);
  rs = null;
  invoke("note_resize", { w, h, done: true }).catch(() => {});
});

listen("note-close", () => paper.classList.add("closing"));
listen("note-flash", () => { paper.classList.remove("flash"); void paper.offsetWidth; paper.classList.add("flash"); });
listen("notes-changed", (e) => {
  const n = (e.payload || []).find((x) => x.id === note?.id);
  if (!n) return;
  // Bu pencerede yazılırken metni ezme
  if (document.activeElement !== text && n.text !== text.value) text.value = n.text;
  note = n;
  paint();
});

(async () => {
  note = await invoke("note_self").catch(() => null);
  if (!note) return;
  build();
  requestAnimationFrame(() => invoke("note_ready").catch(() => {}));
})();

// Tarayıcıda önizleme
async function mockInvoke(cmd) {
  if (cmd === "note_self") return { id: "1", text: "Cuma 15:00 haftalık toplantı\n- Sunumu bitir\n- Faturaları gönder", color: new URLSearchParams(location.search).get("c") || "yellow", top: false, updated: Date.now() };
  return null;
}
