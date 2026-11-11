# Cam Dock — Microsoft Store gönderimi

## Paket
- Actions → son "windows" çalışması → **Artifacts → CamDock-Store** → ZIP içindeki `CamDock_<sürüm>_x64.msix`
- Partner Center → Cam Dock → **Start submission → Packages** → `.msix` dosyasını sürükle (ZIP'i değil)
- Her gönderimde sürüm daha büyük olmalı (CI `tauri.conf.json` sürümünü kullanır, sonuna `.0` ekler)

## Pricing and availability
- Fiyat: **Ücretsiz**
- İlk gönderim: **Private audience / gizli (yalnızca bağlantıyla)** → kendi bilgisayarına Store'dan kur, dene, sonra herkese aç
- Pazarlar: önce Türkiye (arayüz Türkçe), sonra tümü

## Properties
- Kategori: **Utilities & tools** (alt kategori: yok)
- Gizlilik politikası: `https://sifirgecikme.com/camdock/gizlilik`
- Web sitesi: `https://github.com/aliapaydin/cam-dock-releases`
- Destek: `https://sifirgecikme.com/support`
- Sistem gereksinimleri: Windows 11 (64 bit). Klavye ve fare.
- "This product has been tested to meet accessibility guidelines": işaretleme
- "Customers can install this product on alternate drives": işaretleme (tam yetkili uygulama)

## Age ratings (IARC anketi)
- Kategori: **All other app types**
- Şiddet, cinsellik, kumar, uyuşturucu, küfür: **Hayır**
- Kullanıcılar arası iletişim / içerik paylaşımı: **Hayır**
- Konum paylaşımı: **Hayır** (hava durumu için şehir ayarlardan seçilir, paylaşılmaz)
- Dijital satın alma: **Hayır**
- İnternet erişimi: **Evet** (hava durumu, haberler, döviz)
- Beklenen sonuç: 3+ / Everyone

## Submission options → Restricted capabilities (runFullTrust gerekçesi)
> Cam Dock is a desktop taskbar/dock replacement. It needs full trust to: enumerate and activate top-level windows of other apps, hide and restore the classic Windows taskbar (SHAppBarMessage), show system tray icons and forward their menus, read media sessions and audio volume, read the user's own notification database for badges, and run a local server on the private network for the optional phone companion. It does not install drivers or services and makes no changes outside the user's profile; the classic taskbar is restored when the app exits.

## Store listing — Türkçe (tr-TR)

**Ad:** Cam Dock

**Kısa açıklama (≤ 100 karakter):**
Windows 11 için cam görünümlü dock: uygulamalar, widget'lar ve bildirimler tek şeritte.

**Açıklama:**
Cam Dock, Windows 11'in klasik görev çubuğunun yerine geçen, cam görünümlü ve animasyonlu bir dock'tur.

• Mac tarzı büyüteç: simgeler üzerine gelince büyür, komşular yer açar
• Uygulamaların kendi sağ tık menüleri (atlama listeleri) ve tepsi menüleri
• Bildirim rozetleri ve zıplayan simgeler
• Widget'lar: takvim, hava durumu, medya oynatıcı, performans, ekran süresi, döviz ve haberler, pano geçmişi, klasör yığınları, canlı etkinlikler (indirmeler, zamanlayıcı)
• Kendi hızlı ayarlar paneli: Wi-Fi, Bluetooth, ses, parlaklık
• Duvar kâğıdına uyum: renkler ve koyu/açık tema kendiliğinden ayarlanır
• Çoklu ekran desteği
• Telefon ekranı: Android telefonu Wi-Fi ya da USB ile ikinci ekran ve kumanda olarak kullan (saat, hava, müzik, performans, kısayol tuşları)

Hesap gerekmez, reklam yok. İsteğe bağlı anonim kullanım istatistiği ayarlardan kapatılabilir.

Uygulamadan “Çık” dediğinde klasik görev çubuğu geri gelir.

**Özellikler (Product features, her biri ≤ 200 karakter):**
- Mac tarzı büyüteçli cam dock
- Uygulamaların kendi sağ tık menüleri ve tepsi menüleri
- Takvim, hava durumu, medya, performans ve daha fazla widget
- Bildirim rozetleri ve zıplayan simgeler
- Duvar kâğıdına uyan renkler, koyu ve açık tema
- Android telefonu ikinci ekran olarak kullanma

**Anahtar kelimeler:** dock, taskbar, görev çubuğu, widget, mac dock, cam, tema

## Store listing — English (en-US)

**Short description:**
A glass dock for Windows 11: apps, widgets and notifications in one beautiful strip.

**Description:**
Cam Dock replaces the classic Windows 11 taskbar with an animated, glass-style dock. (The app interface is currently in Turkish.)

• Mac-style magnification with neighbouring icons making room
• Each app's own right-click menu (jump lists) and tray icon menus
• Notification badges and bouncing icons
• Widgets: calendar, weather, media player, performance, screen time, markets and news, clipboard history, folder stacks, live activities
• Its own quick settings panel: Wi-Fi, Bluetooth, volume, brightness
• Colors and light/dark theme follow your wallpaper
• Multi-monitor support
• Phone screen: use an Android phone as a second screen and remote over Wi-Fi or USB

No account, no ads. Optional anonymous usage statistics can be turned off in settings. Choosing “Quit” restores the classic taskbar.

## Ekran görüntüleri (1920×1080 ya da daha büyük, PNG)
1. Masaüstü + dock, koyu tema, büyüteç bir simgede açıkken
2. Takvim paneli açık
3. Medya paneli (birden çok oynatıcı) açık
4. Arka planda çalışanlar (tepsi) paneli
5. Bir uygulamanın sağ tık menüsü (atlama listesi)
6. Açık tema + duvar kâğıdı uyumu
(İsteğe bağlı) 7. Telefon ekranı fotoğrafı

İpucu: Win + Shift + S → "Tam ekran" ile çek; 4K ekranda çekilen görüntü de olur.

## Store sürümünde farklı olanlar
- Güncellemeleri Store yapar; uygulamanın kendi güncelleyicisi kapalıdır
- Windows ile başlat, paketin başlangıç görevidir (Görev Yöneticisi → Başlangıç uygulamaları)
- Gezgin'deki "Cam Dock'a sabitle" menüsü yok
- Kaldırmadan önce Cam Dock'tan "Çık" demek önerilir (klasik görev çubuğu geri gelir)
