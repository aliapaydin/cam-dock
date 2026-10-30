// Takvim yardımcıları: Türkiye resmî tatilleri, hafta numarası, biçimler.
// Hem dock (index.html) hem panel (popover.html) kullanır.
"use strict";

const CAL = (() => {
  // Sabit tarihli resmî tatiller (ay, gün, ad, yarım gün mü)
  const FIXED = [
    [1, 1, "Yılbaşı"],
    [4, 23, "Ulusal Egemenlik ve Çocuk Bayramı"],
    [5, 1, "Emek ve Dayanışma Günü"],
    [5, 19, "Atatürk'ü Anma, Gençlik ve Spor Bayramı"],
    [7, 15, "Demokrasi ve Millî Birlik Günü"],
    [8, 30, "Zafer Bayramı"],
    [10, 28, "Cumhuriyet Bayramı arifesi", true],
    [10, 29, "Cumhuriyet Bayramı"],
  ];
  // Dinî bayramlar (Diyanet takvimi). Arife yarım gün.
  const RELIGIOUS = {
    2026: [
      ["2026-03-19", "Ramazan Bayramı arifesi", true],
      ["2026-03-20", "Ramazan Bayramı 1. gün"],
      ["2026-03-21", "Ramazan Bayramı 2. gün"],
      ["2026-03-22", "Ramazan Bayramı 3. gün"],
      ["2026-05-26", "Kurban Bayramı arifesi", true],
      ["2026-05-27", "Kurban Bayramı 1. gün"],
      ["2026-05-28", "Kurban Bayramı 2. gün"],
      ["2026-05-29", "Kurban Bayramı 3. gün"],
      ["2026-05-30", "Kurban Bayramı 4. gün"],
    ],
    2027: [
      ["2027-03-08", "Ramazan Bayramı arifesi", true],
      ["2027-03-09", "Ramazan Bayramı 1. gün"],
      ["2027-03-10", "Ramazan Bayramı 2. gün"],
      ["2027-03-11", "Ramazan Bayramı 3. gün"],
      ["2027-05-15", "Kurban Bayramı arifesi", true],
      ["2027-05-16", "Kurban Bayramı 1. gün"],
      ["2027-05-17", "Kurban Bayramı 2. gün"],
      ["2027-05-18", "Kurban Bayramı 3. gün"],
      ["2027-05-19", "Kurban Bayramı 4. gün"],
    ],
  };

  const pad = (n) => String(n).padStart(2, "0");
  const key = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const cache = new Map();

  function holidaysOf(year) {
    if (cache.has(year)) return cache.get(year);
    const map = new Map();
    const add = (k, name, half) => {
      const prev = map.get(k);
      map.set(k, prev ? { name: `${prev.name} · ${name}`, half: prev.half && half } : { name, half: !!half });
    };
    for (const [m, d, name, half] of FIXED) add(`${year}-${pad(m)}-${pad(d)}`, name, half);
    for (const [k, name, half] of RELIGIOUS[year] || []) add(k, name, half);
    cache.set(year, map);
    return map;
  }

  const holiday = (d) => holidaysOf(d.getFullYear()).get(key(d)) || null;

  // ISO 8601 hafta numarası (pazartesi başlangıçlı)
  function isoWeek(d) {
    const t = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
    const day = t.getUTCDay() || 7;
    t.setUTCDate(t.getUTCDate() + 4 - day);
    const y0 = new Date(Date.UTC(t.getUTCFullYear(), 0, 1));
    return Math.ceil(((t - y0) / 86400000 + 1) / 7);
  }

  const dayOfYear = (d) =>
    Math.round((new Date(d.getFullYear(), d.getMonth(), d.getDate()) - new Date(d.getFullYear(), 0, 1)) / 86400000) + 1;

  const sameDay = (a, b) => key(a) === key(b);
  const startOfDay = (d) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const daysBetween = (a, b) => Math.round((startOfDay(b) - startOfDay(a)) / 86400000);

  // Bugünden sonraki tatiller (arifeler dahil değil, bayramın ilk günü yeter)
  function upcoming(from, count = 4) {
    const out = [];
    for (const y of [from.getFullYear(), from.getFullYear() + 1]) {
      for (const [k, h] of holidaysOf(y)) {
        if (h.half) continue;
        const [yy, mm, dd] = k.split("-").map(Number);
        const d = new Date(yy, mm - 1, dd);
        if (daysBetween(from, d) <= 0) continue;
        // Çok günlü bayramlarda yalnızca 1. günü göster
        if (/ [2-4]\. gün$/.test(h.name)) continue;
        out.push({ date: d, name: h.name.replace(/ 1\. gün$/, ""), days: daysBetween(from, d) });
      }
    }
    out.sort((a, b) => a.date - b.date);
    return out.slice(0, count);
  }

  const MONTHS = ["Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran", "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık"];
  const WEEKDAYS = ["Pzt", "Sal", "Çar", "Per", "Cum", "Cmt", "Paz"];
  const DAYNAMES = ["Pazar", "Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi"];

  return { holiday, isoWeek, dayOfYear, sameDay, daysBetween, upcoming, MONTHS, WEEKDAYS, DAYNAMES, key };
})();
