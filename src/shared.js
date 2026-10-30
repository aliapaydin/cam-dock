// Dock ve panel pencerelerinin ortak yardımcıları: piyasa tanımları, haber
// akışı ayrıştırma, sayı/zaman biçimleri.
"use strict";

const MARKETS = {
  USD: { name: "Dolar", pair: "USD/TRY", glyph: "$", bg: "#1E9E63" },
  EUR: { name: "Euro", pair: "EUR/TRY", glyph: "€", bg: "#2F7BFF" },
  GBP: { name: "Sterlin", pair: "GBP/TRY", glyph: "£", bg: "#7C5CFF" },
  CHF: { name: "İsviçre frangı", pair: "CHF/TRY", glyph: "₣", bg: "#E5484D" },
  "gram-altin": { name: "Gram altın", pair: "TL", glyph: "g", bg: "linear-gradient(135deg,#F6C744,#B7791F)" },
  "ceyrek-altin": { name: "Çeyrek altın", pair: "TL", glyph: "¼", bg: "linear-gradient(135deg,#F6C744,#B7791F)" },
};
const NEWS_ICON = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 5h13v14H6a2 2 0 0 1-2-2z"/><path d="M17 9h3v8a2 2 0 0 1-2 2"/><path d="M8 9h5M8 13h5"/></svg>';

const trNum = (s) => Number(String(s).replace(/[%$\s]/g, "").replace(/\./g, "").replace(",", "."));
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

// Panelde gösterilen tüm piyasalar (dock'ta yalnızca ayarlarda seçilenler döner).
const MARKET_GROUPS = [
  ["Döviz", ["USD", "EUR", "GBP", "CHF", "JPY", "SAR"]],
  ["Altın ve değerli metaller", ["gram-altin", "ceyrek-altin", "yarim-altin", "tam-altin", "cumhuriyet-altini", "ons", "gumus"]],
];
const MARKET_EXTRA = {
  JPY: { name: "Japon yeni", pair: "JPY/TRY", glyph: "¥", bg: "#C2410C" },
  SAR: { name: "Suudi riyali", pair: "SAR/TRY", glyph: "﷼", bg: "#0F766E" },
  "yarim-altin": { name: "Yarım altın", pair: "TL", glyph: "½", bg: "linear-gradient(135deg,#F6C744,#B7791F)" },
  "tam-altin": { name: "Tam altın", pair: "TL", glyph: "1", bg: "linear-gradient(135deg,#F6C744,#B7791F)" },
  "cumhuriyet-altini": { name: "Cumhuriyet altını", pair: "TL", glyph: "C", bg: "linear-gradient(135deg,#F6C744,#B7791F)" },
  ons: { name: "Ons altın", pair: "USD", glyph: "oz", bg: "linear-gradient(135deg,#E0B43A,#8A5A12)" },
  gumus: { name: "Gram gümüş", pair: "TL", glyph: "Ag", bg: "linear-gradient(135deg,#CBD5E1,#64748B)" },
};
const marketInfo = (k) => MARKETS[k] || MARKET_EXTRA[k];

// Google Haberler konu akışları (Türkçe)
const GN = (topic) => topic
  ? `https://news.google.com/rss/headlines/section/topic/${topic}?hl=tr&gl=TR&ceid=TR:tr`
  : "https://news.google.com/rss?hl=tr&gl=TR&ceid=TR:tr";
const NEWS_CATEGORIES = [
  { id: "top", name: "Gündem", url: GN() },
  { id: "nation", name: "Türkiye", url: GN("NATION") },
  { id: "world", name: "Dünya", url: GN("WORLD") },
  { id: "business", name: "Ekonomi", url: GN("BUSINESS") },
  { id: "tech", name: "Teknoloji", url: GN("TECHNOLOGY") },
  { id: "sports", name: "Spor", url: GN("SPORTS") },
  { id: "science", name: "Bilim", url: GN("SCIENCE") },
  { id: "health", name: "Sağlık", url: GN("HEALTH") },
];

// ---------------------------------------------------------------------------
// Dinamik mod: duvar kâğıdından yumuşak renkler
// ---------------------------------------------------------------------------
// Renkler hiçbir zaman ayarlara yazılmaz; yalnızca pencerenin kökünde CSS
// değişkenleri olarak uygulanır. Dinamik mod kapanınca değişkenler silinir ve
// kullanıcının kendi teması/vurgu rengi aynen geri gelir.
const Dyn = (() => {
  let sig = null;
  let pal = null;
  const applied = new Set();

  const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
  function rgbToHsl(r, g, b) {
    r /= 255; g /= 255; b /= 255;
    const max = Math.max(r, g, b), min = Math.min(r, g, b);
    const l = (max + min) / 2;
    if (max === min) return [0, 0, l];
    const d = max - min;
    const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    let h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
    return [h * 60, s, l];
  }
  function hslToRgb(h, s, l) {
    const k = (n) => (n + h / 30) % 12;
    const a = s * Math.min(l, 1 - l);
    const f = (n) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
    return [f(0), f(8), f(4)].map((x) => Math.round(x * 255));
  }
  const mix = (a, b, t) => a.map((v, i) => Math.round(v + (b[i] - v) * t));
  const rgba = (c, a) => `rgba(${c[0]}, ${c[1]}, ${c[2]}, ${a})`;

  // Baskın canlı renk: 10°'lik renk tonu kovaları, doygun ve orta açıklıktaki
  // pikseller daha ağır sayılır. Siyah-beyaz duvar kâğıdında `gray` döner.
  function analyse(px) {
    const bins = Array.from({ length: 36 }, () => ({ w: 0, x: 0, y: 0, s: 0, l: 0 }));
    let total = 0, lum = 0, n = 0;
    for (let i = 0; i < px.length; i += 4) {
      const [h, s, l] = rgbToHsl(px[i], px[i + 1], px[i + 2]);
      lum += l; n++;
      const w = s * s * Math.max(0, 1 - Math.abs(l - 0.52) * 1.9);
      if (w <= 0.002) continue;
      const b = bins[Math.floor(h / 10) % 36];
      const rad = (h * Math.PI) / 180;
      b.w += w; b.x += Math.cos(rad) * w; b.y += Math.sin(rad) * w; b.s += s * w; b.l += l * w;
      total += w;
    }
    // Komşu kovalarla birlikte en ağır bölge
    let best = 0, bestW = -1;
    for (let i = 0; i < 36; i++) {
      const w = bins[i].w + 0.6 * (bins[(i + 35) % 36].w + bins[(i + 1) % 36].w);
      if (w > bestW) { bestW = w; best = i; }
    }
    const group = [bins[(best + 35) % 36], bins[best], bins[(best + 1) % 36]];
    const W = group.reduce((a, b) => a + b.w, 0);
    const hue = W ? ((Math.atan2(group.reduce((a, b) => a + b.y, 0), group.reduce((a, b) => a + b.x, 0)) * 180) / Math.PI + 360) % 360 : 220;
    return {
      hue,
      sat: W ? group.reduce((a, b) => a + b.s, 0) / W : 0,
      gray: total / Math.max(1, n) < 0.012,
      lum: lum / Math.max(1, n),
    };
  }

  async function fromImage(url) {
    const img = new Image();
    img.src = url;
    await img.decode();
    const c = document.createElement("canvas");
    c.width = 64; c.height = 36;
    const ctx = c.getContext("2d", { willReadFrequently: true });
    ctx.drawImage(img, 0, 0, 64, 36);
    return analyse(ctx.getImageData(0, 0, 64, 36).data);
  }
  function fromSolid([r, g, b]) {
    const [h, s] = rgbToHsl(r, g, b);
    return { hue: h, sat: s, gray: s < 0.12, lum: rgbToHsl(r, g, b)[2] };
  }

  /** Duvar kâğıdını (değiştiyse) yeniden okur. Değiştiyse true. */
  async function refresh(force) {
    const w = await invoke("wallpaper", { known: force ? null : sig }).catch(() => null);
    if (!w || !w.sig) return false;
    if (w.sig === sig && pal) return false;
    try {
      pal = w.solid ? fromSolid(w.solid) : w.image ? await fromImage(w.image) : pal;
      sig = w.sig;
      return true;
    } catch (e) {
      console.warn("duvar kâğıdı okunamadı", e);
      return false;
    }
  }

  // Pencere türüne göre değişkenler. `amt`: 0–1 karışım oranı.
  function vars(kind, dark, amt) {
    const h = pal.hue;
    const s = pal.gray ? 0 : clamp(pal.sat, 0.25, 0.85);
    const accent = pal.gray ? null : hslToRgb(h, clamp(s, 0.5, 0.82), dark ? 0.63 : 0.47);
    const baseDark = [16, 20, 34], baseLight = [246, 248, 252];
    const glass = dark ? mix(baseDark, hslToRgb(h, s * 0.6, 0.17), amt) : mix(baseLight, hslToRgb(h, s * 0.55, 0.91), amt);
    const cellTone = hslToRgb(h, s * 0.5, dark ? 0.72 : 0.97);
    const m = { __tint: glass };
    if (accent) {
      m["--accent"] = `rgb(${accent.join(", ")})`;
    }
    if (kind === "dock") {
      m["--dock-bg"] = rgba(glass, dark ? 0.44 : 0.68);
      m["--dock-border"] = dark ? rgba(mix([255, 255, 255], cellTone, amt * 0.6), 0.13) : "rgba(255, 255, 255, 0.32)";
      m["--cell-bg"] = dark ? rgba(cellTone, 0.06) : rgba(mix([255, 255, 255], cellTone, amt), 0.55);
      m["--cell-hover"] = dark ? rgba(cellTone, 0.1) : rgba(mix([255, 255, 255], cellTone, amt), 0.74);
      m["--cell-active-bg"] = dark ? rgba(cellTone, 0.13) : rgba(mix([255, 255, 255], cellTone, amt * 0.5), 0.9);
      if (accent) {
        m["--cell-active-border"] = rgba(accent, 0.45);
        m["--chart"] = `rgb(${accent.join(", ")})`;
        m["--chart-dim"] = rgba(accent, 0.45);
      }
    } else if (kind === "popover") {
      m["--bg"] = rgba(glass, dark ? 0.5 : 0.76);
      m["--cell"] = dark ? rgba(cellTone, 0.06) : rgba(mix([255, 255, 255], cellTone, amt), 0.55);
      m["--hover"] = dark ? rgba(cellTone, 0.1) : rgba(mix([255, 255, 255], cellTone, amt), 0.85);
      if (accent) {
        m["--bar"] = `rgb(${accent.join(", ")})`;
        m["--bar-dim"] = rgba(accent, 0.38);
      }
    }
    return m;
  }

  function clear(root) {
    for (const k of applied) root.style.removeProperty(k);
    applied.clear();
    delete root.dataset.dynamic;
  }

  /** Dinamik mod açıksa değişkenleri uygular; dönüş: cam rengi [r,g,b] ya da null. */
  async function apply(root, cfg, dark, kind) {
    if (!cfg?.dynamic?.enabled) { clear(root); return null; }
    if (!pal) await refresh();
    if (!pal) { clear(root); return null; }
    const amt = clamp((cfg.dynamic.tint ?? 45) / 100, 0, 1) * 0.6;
    const m = vars(kind, dark, amt);
    const tint = m.__tint;
    delete m.__tint;
    for (const k of applied) if (!(k in m)) root.style.removeProperty(k);
    applied.clear();
    for (const [k, v] of Object.entries(m)) { root.style.setProperty(k, v); applied.add(k); }
    root.dataset.dynamic = "1";
    return tint;
  }

  /** Duvar kâğıdı değişince `onChange` çağrılır (dinamik mod açıkken). */
  function watch(getCfg, onChange) {
    setInterval(async () => {
      if (getCfg()?.dynamic?.enabled && (await refresh())) onChange();
    }, 30_000);
  }

  /** Tema değişirken renkler yumuşakça geçsin. */
  function animate(root) {
    root.classList.add("retheme");
    clearTimeout(root._rt);
    root._rt = setTimeout(() => root.classList.remove("retheme"), 1300);
  }

  /** Kapak görselinden iki canlı renk (medya kartındaki dalga için). */
  const artCache = new Map();
  async function artColors(url) {
    if (!url) return null;
    if (artCache.has(url)) return artCache.get(url);
    let out = null;
    try {
      const p = await fromImage(url);
      const s = p.gray ? 0.15 : clamp(p.sat, 0.45, 0.85);
      out = {
        c1: `hsl(${Math.round(p.hue)} ${Math.round(s * 100)}% 58%)`,
        c2: `hsl(${Math.round((p.hue + 40) % 360)} ${Math.round(s * 90)}% 52%)`,
      };
    } catch { out = null; }
    artCache.set(url, out);
    return out;
  }

  return { apply, refresh, watch, animate, artColors, preview: () => pal };
})();

// ---------------------------------------------------------------------------
// Hava durumu (Open-Meteo, anahtarsız)
// ---------------------------------------------------------------------------
const WX = {
  0: ["Açık", "clear"], 1: ["Çoğunlukla açık", "mostly"], 2: ["Parçalı bulutlu", "partly"], 3: ["Kapalı", "cloud"],
  45: ["Sisli", "fog"], 48: ["Kırağılı sis", "fog"],
  51: ["Hafif çisenti", "drizzle"], 53: ["Çisenti", "drizzle"], 55: ["Yoğun çisenti", "drizzle"],
  56: ["Dondurucu çisenti", "sleet"], 57: ["Dondurucu çisenti", "sleet"],
  61: ["Hafif yağmur", "rain"], 63: ["Yağmurlu", "rain"], 65: ["Şiddetli yağmur", "rain"],
  66: ["Dondurucu yağmur", "sleet"], 67: ["Dondurucu yağmur", "sleet"],
  71: ["Hafif kar", "snow"], 73: ["Karlı", "snow"], 75: ["Yoğun kar", "snow"], 77: ["Kar taneleri", "snow"],
  80: ["Hafif sağanak", "rain"], 81: ["Sağanak", "rain"], 82: ["Şiddetli sağanak", "rain"],
  85: ["Kar sağanağı", "snow"], 86: ["Yoğun kar sağanağı", "snow"],
  95: ["Gök gürültülü", "storm"], 96: ["Dolulu fırtına", "storm"], 99: ["Dolulu fırtına", "storm"],
};
const wxText = (code) => (WX[code] || ["–", "cloud"])[0];

// Çok renkli, sade simgeler (24×24)
function wxIcon(code, day = true, size = 24) {
  const kind = (WX[code] || ["", "cloud"])[1];
  const sun = (cx, cy, r) => `<g class="wx-sun"><circle cx="${cx}" cy="${cy}" r="${r}" fill="#FDB833"/>${[0, 45, 90, 135, 180, 225, 270, 315]
    .map((a) => { const t = (a * Math.PI) / 180; return `<line x1="${cx + Math.cos(t) * (r + 2.2)}" y1="${cy + Math.sin(t) * (r + 2.2)}" x2="${cx + Math.cos(t) * (r + 4)}" y2="${cy + Math.sin(t) * (r + 4)}" stroke="#FDB833" stroke-width="1.8" stroke-linecap="round"/>`; }).join("")}</g>`;
  const moon = (cx, cy, r) => `<path d="M${cx + r * 0.35} ${cy - r} a${r} ${r} 0 1 0 ${r * 0.65} ${r * 1.55} a${r * 0.8} ${r * 0.8} 0 0 1 -${r * 0.65} -${r * 1.55}z" fill="#C9D4F2"/>`;
  const cloud = (dx = 0, dy = 0, fill = "var(--wx-cloud, #DCE3EE)") =>
    `<path transform="translate(${dx} ${dy})" d="M7.5 19h9.2a3.8 3.8 0 0 0 .5-7.57A5.2 5.2 0 0 0 7.2 10.6 4.2 4.2 0 0 0 7.5 19z" fill="${fill}"/>`;
  const drops = (n, color = "#4FA3FF") => Array.from({ length: n }, (_, i) => `<line x1="${8.5 + i * 3.5}" y1="20.5" x2="${7.5 + i * 3.5}" y2="23" stroke="${color}" stroke-width="1.8" stroke-linecap="round"/>`).join("");
  const flakes = () => [8.5, 12, 15.5].map((x) => `<circle cx="${x}" cy="21.6" r="1.2" fill="#E8F1FF"/>`).join("");
  let body;
  switch (kind) {
    case "clear": body = day ? sun(12, 12, 4.6) : moon(12, 12, 6.5); break;
    case "mostly": body = (day ? sun(10, 10, 4.2) : moon(10, 10, 5.5)) + cloud(3, 3.5); break;
    case "partly": body = (day ? sun(8.5, 8.5, 3.6) : moon(9, 8.5, 4.8)) + cloud(1.5, 1); break;
    case "cloud": body = cloud(-1.5, -4, "var(--wx-cloud2, #AEB9CA)") + cloud(1, -1); break;
    case "fog": body = cloud(0, -4) + [15, 18, 21].map((y, i) => `<line x1="${4 + i}" y1="${y}" x2="${20 - i}" y2="${y}" stroke="#AEB9CA" stroke-width="1.7" stroke-linecap="round"/>`).join(""); break;
    case "drizzle": body = cloud(0, -3.5) + drops(3, "#7FBFFF"); break;
    case "rain": body = cloud(0, -3.5) + drops(3); break;
    case "sleet": body = cloud(0, -3.5) + drops(2) + `<circle cx="15.5" cy="21.6" r="1.2" fill="#E8F1FF"/>`; break;
    case "snow": body = cloud(0, -3.5) + flakes(); break;
    case "storm": body = cloud(0, -3.5, "var(--wx-cloud2, #AEB9CA)") + `<path d="M12.5 15.5l-2.5 4h2.6l-1.6 3.8 4.3-5.2h-2.6l1.6-2.6z" fill="#FDB833"/>`; break;
    default: body = cloud();
  }
  return `<svg class="wx-ic" width="${size}" height="${size}" viewBox="0 0 24 24" aria-hidden="true">${body}</svg>`;
}

const Weather = (() => {
  let cache = null; // { at, key, data }
  async function locate(cfg) {
    const w = cfg.weather || {};
    if (!w.auto_location) return { lat: w.lat, lon: w.lon, city: w.city };
    try {
      const g = JSON.parse(await invoke("fetch_text", { url: "https://get.geojs.io/v1/ip/geo.json" }));
      const lat = Number(g.latitude), lon = Number(g.longitude);
      if (isFinite(lat) && isFinite(lon)) {
        const loc = { lat, lon, city: g.city || g.region || w.city };
        // Konumu kaydet: gün doğumu/batımı (dinamik tema) da bunu kullanır.
        if (Math.abs(lat - w.lat) > 0.05 || Math.abs(lon - w.lon) > 0.05 || loc.city !== w.city) {
          invoke("set_weather_location", { city: loc.city, lat, lon }).catch(() => {});
          Object.assign(w, { lat, lon, city: loc.city });
        }
        return loc;
      }
    } catch (e) { console.warn("konum bulunamadı", e); }
    return { lat: w.lat, lon: w.lon, city: w.city };
  }
  async function get(cfg, force) {
    const loc = await locate(cfg);
    const key = `${loc.lat.toFixed(2)},${loc.lon.toFixed(2)}`;
    if (!force && cache && cache.key === key && Date.now() - cache.at < 10 * 60_000) return cache.data;
    const url = "https://api.open-meteo.com/v1/forecast?latitude=" + loc.lat + "&longitude=" + loc.lon +
      "&current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,is_day,wind_speed_10m,precipitation" +
      "&hourly=temperature_2m,weather_code,precipitation_probability,is_day" +
      "&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,sunrise,sunset,uv_index_max" +
      "&timezone=auto&forecast_days=6";
    const d = JSON.parse(await invoke("fetch_text", { url }));
    d.city = loc.city;
    cache = { at: Date.now(), key, data: d };
    return d;
  }
  return { get };
})();

// ---------------------------------------------------------------------------
// Tarayıcı önizlemesi için örnek veri (Tauri dışında açılınca kullanılır)
// ---------------------------------------------------------------------------
const MOCK = {
  wallpaper(kind) {
    const c = document.createElement("canvas");
    c.width = 320; c.height = 180;
    const g = c.getContext("2d");
    const sets = {
      sunset: ["#1b1035", "#7a2e5f", "#f08a4b"], forest: ["#0d2418", "#2f6b3f", "#a9c97a"],
      ocean: ["#06233d", "#0f6b8f", "#7fd3e6"], gray: ["#222", "#666", "#bbb"],
    };
    const cols = sets[kind] || sets.sunset;
    const grd = g.createLinearGradient(0, 0, 320, 180);
    cols.forEach((col, i) => grd.addColorStop(i / (cols.length - 1), col));
    g.fillStyle = grd;
    g.fillRect(0, 0, 320, 180);
    return { sig: kind || "sunset", image: c.toDataURL("image/png"), solid: null };
  },
  fetch(url) {
    if (url.includes("geojs")) return JSON.stringify({ latitude: "38.42", longitude: "27.14", city: "İzmir" });
    const now = new Date();
    const pad = (n) => String(n).padStart(2, "0");
    const iso = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:00`;
    const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const hourly = { time: [], temperature_2m: [], weather_code: [], precipitation_probability: [], is_day: [] };
    const codes = [0, 1, 2, 3, 61, 80, 2, 1];
    for (let i = 0; i < 144; i++) {
      const d = new Date(start.getTime() + i * 3600e3);
      const h = d.getHours();
      hourly.time.push(iso(d));
      hourly.temperature_2m.push(Math.round((14 + 6 * Math.sin(((h - 9) / 24) * 2 * Math.PI) + Math.sin(i / 17) * 2) * 10) / 10);
      hourly.weather_code.push(codes[Math.floor(i / 9) % codes.length]);
      hourly.precipitation_probability.push(Math.max(0, Math.round(40 * Math.sin(i / 7) + 10)));
      hourly.is_day.push(h >= 7 && h < 18 ? 1 : 0);
    }
    const daily = { time: [], weather_code: [1, 61, 3, 0, 80, 2], temperature_2m_max: [19, 16, 17, 21, 18, 20], temperature_2m_min: [11, 10, 9, 12, 13, 12], precipitation_probability_max: [5, 80, 30, 0, 65, 10], sunrise: [], sunset: [], uv_index_max: [3.1, 1.8, 2.5, 3.9, 2.2, 3.4] };
    for (let i = 0; i < 6; i++) {
      const d = new Date(start.getTime() + i * 86400e3);
      daily.time.push(iso(d).slice(0, 10));
      daily.sunrise.push(`${iso(d).slice(0, 10)}T07:${pad(38 + i)}`);
      daily.sunset.push(`${iso(d).slice(0, 10)}T18:${pad(14 - i)}`);
    }
    return JSON.stringify({
      current: { temperature_2m: 17.6, apparent_temperature: 16.4, relative_humidity_2m: 64, weather_code: 2, is_day: now.getHours() >= 7 && now.getHours() < 18 ? 1 : 0, wind_speed_10m: 14.2, precipitation: 0 },
      hourly, daily,
    });
  },
};
