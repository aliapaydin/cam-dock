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
