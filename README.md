# Cam Dock — v0.5

Windows 11'deki klasik taskbar'ın yerine geçen cam dock. Tauri 2 (Rust + WebView2) ile yazıldı.

## Bu sürümde çalışanlar

| Parça | Durum |
|---|---|
| Klasik taskbar'ı gizleme, çıkışta geri getirme | ✅ |
| Dock için ekranın altında alan ayırma (maximize pencereler dock'un altına girmez) | ✅ |
| Gerçek cam efekti (Windows Acrylic) + koyu/açık tema, sistem temasını takip | ✅ |
| Sabitlenmiş uygulamalar + açık pencereler, gerçek ikonlar, tıkla öne getir/küçült | ✅ |
| Başlat düğmesi (Başlat menüsünü açar) | ✅ |
| Müzik: Spotify, YouTube, Apple Music… (Windows medya oturumu), kapak, oynat/duraklat/ileri/geri | ✅ |
| Ekran süresi (2 dk'dan uzun boşta kalınan süre sayılmaz) + saatlik çubuklar | ✅ |
| **Piyasa ve haberler**: dolar, euro, gram altın (Truncgil) ve RSS haber başlıkları dönüşümlü kayar; habere tıklayınca tarayıcıda açılır | ✅ |
| Tarih / saat | ✅ |
| Tam ekran oyun/video açılınca dock'un kendini gizlemesi | ✅ |
| **Ayarlar penceresi** (sağ tık → Ayarlar…): görünüm, dock/taskbar, ekranlar, uygulamalar, widget'lar | ✅ |
| **Dock görünürlüğü**: her zaman görünür / otomatik gizle / akıllı gizle, kayarak açılıp kapanma | ✅ |
| **Çoklu ekran**: sadece ana ekran / tüm ekranlar / seçili ekranlar, ekran takıp çıkarınca otomatik uyum | ✅ |
| Her dock yalnızca kendi ekranındaki pencereleri gösterebilir | ✅ |
| Başlat menüsünden arayıp uygulama sabitleme, sıralama | ✅ |
| Takvim etkinlikleri, mail, Claude kullanımı | ⏳ Faz 2 (mail ve Claude şimdilik örnek verili önizleme olarak açılabilir) |
| Açılır paneller (takvim, müzik) | ⏳ Faz 2 |

## Kurulum (bir kerelik)

1. **Rust**: https://rustup.rs → `rustup-init.exe`. Kurulum "Visual Studio C++ Build Tools" isterse kabul et ("C++ ile masaüstü geliştirme").
2. **Node.js** 20 veya üstü.
3. WebView2 Windows 11'de zaten yüklü.

## Çalıştırma

```powershell
cd cam-dock
npm install
npm run dev      # geliştirme modu (ilk derleme birkaç dakika sürer)
npm run build    # kurulum dosyası: src-tauri\target\release\bundle\nsis\
```

Dock açılınca klasik taskbar kaybolur. **Dock'a sağ tıkla → Ayarlar…** ile her şeyi ayarlayabilirsin. Değişiklikler anında uygulanır.

## Ayarlar

Ayarlar penceresi şu dosyaya yazar: `%APPDATA%\com.ali.camdock\config.json` (elle de düzenlenebilir).

### Dock görünürlüğü
- **Her zaman görünür** — dock hep ekranda; "alan ayır" açıksa maximize pencereler dock'un üstünde biter.
- **Otomatik gizle** — fareyi ekranın en altına götürünce kayarak açılır, ayrılınca (gecikme sonrası) gizlenir.
- **Akıllı gizle** — öndeki pencere dock'un alanına değerse çekilir, yer açıksa görünür kalır; ekranın altına gidince yine açılır.

### Çift ekran
- **Sadece ana ekranda** — diğer ekranlarda klasik taskbar otomatik gizlenen halde çalışır.
- **Tüm ekranlarda** — her ekranın altında ayrı dock; her biri kendi ekranında ortalanır, kendi alanını ayırır, tam ekranda yalnızca o ekrandaki dock çekilir.
- **Seçtiğim ekranlarda** — Ayarlar → Ekranlar'daki çizimde ekrana tıklayarak aç/kapat.
- "Her dock kendi ekranındaki uygulamaları göstersin" açıksa her dock sadece o ekrandaki pencereleri listeler (sabitlenenler hepsinde).

```jsonc
{
  "theme": "system",          // system | dark | light
  "backdrop": "acrylic",      // acrylic | blur | none
  "hide_taskbar": true,
  "dock_visibility": "always", // always | autohide | smart
  "hide_delay_ms": 600,
  "hide_in_fullscreen": true,
  "display": { "mode": "primary", "selected": [], "apps_per_monitor": false }, // primary | all | selected
  "reserve_space": true,      // maximize pencereler dock'un üstünde biter (sadece always)
  "bottom_margin": 10,
  "radius": 26,
  "accent": "#2F7BFF",
  "show_running": true,       // sabitlenmemiş açık uygulamaları da göster
  "pinned": [
    { "name": "Chrome", "path": "%ProgramFiles%\\Google\\Chrome\\Application\\chrome.exe" }
  ],
  "widgets": ["calendar", "screentime", "ticker", "media", "clock"],
  "ticker": {
    "markets": ["USD", "EUR", "gram-altin"],   // + GBP, CHF, ceyrek-altin
    "news": true,
    "feeds": ["https://news.google.com/rss?hl=tr&gl=TR&ceid=TR:tr"],
    "interval_sec": 6,
    "news_per_round": 3
  }
}
```

- `pinned` → exe, `.lnk` kısayol, klasör ya da `ms-settings:` gibi bir adres olabilir. Bulunamayan yollar gizlenir.
- `widgets` → sıra burada belirlenir. Örnek verili önizlemeler: `"mail"`, `"claude"`.
- Uygulama ikonu: **sol tık** öne getir / küçült (birden fazla pencere varsa sırayla gezer), **orta tık** yeni pencere.

## Taskbar geri gelmezse

Uygulama çıkarken taskbar'ı geri getirir; çökse bile durum dosyaya yazılır. Yine de:

```powershell
cam-dock.exe --restore-taskbar
```

Elle: Ayarlar → Kişiselleştirme → Görev çubuğu → Görev çubuğu davranışları → "Görev çubuğunu otomatik olarak gizle" kapat.

## Bilinen sınırlamalar

- **Sistem düğmeleri** (saatin solunda): gizli simgeler (tepsi taşma menüsü), hızlı ayarlar (`Win + A`), bildirimler (`Win + N`) ve klasik taskbar'ı birkaç saniye gösterme. Taskbar, üzerinde ya da menüsü açıkken gizlenmez.
- Cam efekti Windows Composition ile yapılıyor (`HostBackdropBrush` + yuvarlak kırpma), böylece köşeler gerçekten yuvarlak. Kurulamazsa eski yönteme (`SetWindowCompositionAttribute`) düşer; bu durum tanılama kaydına yazılır.
- Windows ayarlarında **Saydamlık efektleri** kapalıysa cam görünmez.
- Tanılama kaydı: `%APPDATA%\com.ali.camdock\dock.log` (Ayarlar → Hakkında → Tanılama kaydı).
- Şirket bilgisayarlarında grup ilkeleri taskbar'ın gizlenmesini engelleyebilir.

## Yapı

```
src/                 Arayüz: dock (index.html) ve ayarlar (settings.html) — bundler yok
src-tauri/           Tauri uygulaması: komutlar, ayarlar, arka plan izleyici
crates/dock-sys/     Windows API katmanı: taskbar, AppBar, acrylic, pencereler, ikonlar, medya
.github/workflows/   GitHub'a atılırsa Windows'ta otomatik derleyip .exe üretir
```

Tarayıcıda hızlı önizleme (örnek verilerle): `src/index.html?theme=dark`, `src/settings.html?theme=light&tab=displays`.

## Geliştirici

Ali Apaydın — [GitHub](https://github.com/aliapaydin/cam-dock) · apaydin.a@gmail.com · [sifirgecikme.com](https://sifirgecikme.com)
