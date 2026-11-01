//! Windows medya oturumu (SMTC): Spotify, YouTube (tarayıcı), Apple Music vb.
//! Hangi uygulama çalıyorsa onun bilgisi ve kontrolleri.

use serde::Serialize;
use windows::core::Result;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;
use windows_future::{AsyncStatus, IAsyncOperation};

/// WinRT işlemini en fazla `ms` milisaniye bekler. Bazı oynatıcılar (ör. donmuş
/// bir tarayıcı sekmesi) hiç cevap vermeyebilir; `join()` sonsuza kadar bekleyip
/// iş parçacığını kilitliyordu.
pub(crate) fn wait<T: windows_core::RuntimeType + 'static>(op: IAsyncOperation<T>, ms: u64) -> Result<T> {
    let start = std::time::Instant::now();
    loop {
        match op.Status()? {
            AsyncStatus::Completed => return op.GetResults(),
            AsyncStatus::Started => {}
            _ => return Err(windows_core::Error::from_hresult(windows_core::HRESULT(0x80004005u32 as i32))),
        }
        if start.elapsed().as_millis() as u64 >= ms {
            let _ = op.Cancel();
            return Err(windows_core::Error::from_hresult(windows_core::HRESULT(0x800705B4u32 as i32)));
        }
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MediaState {
    /// Oturum kimliği: kaynak uygulama + sıra ("Spotify.exe#0")
    pub id: String,
    /// Windows'un "şu anki" saydığı oturum mu
    pub current: bool,
    pub active: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    /// Kaynak uygulama kimliği (ör. Spotify.exe)
    pub source: String,
    /// Kapak değişti mi anlamak için anahtar
    pub track_key: String,
}

static MANAGER: std::sync::Mutex<Option<Manager>> = std::sync::Mutex::new(None);

/// Medya yöneticisini bir kez alıp saklarız (her saniye yeniden istemek pahalı).
fn manager() -> Result<Manager> {
    let mut slot = MANAGER.lock().unwrap();
    if let Some(m) = slot.as_ref() {
        return Ok(m.clone());
    }
    let m = wait(Manager::RequestAsync()?, 3000)?;
    *slot = Some(m.clone());
    Ok(m)
}

fn session() -> Result<Option<Session>> {
    let mgr = manager()?;
    match mgr.GetCurrentSession() {
        Ok(s) => Ok(Some(s)),
        Err(_) => Ok(None),
    }
}

/// 1601'den beri 100 ns cinsinden şu an (Windows DateTime ölçeği).
fn now_ticks() -> i64 {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64 / 100)
        .unwrap_or(0);
    unix + 116_444_736_000_000_000
}

pub fn state() -> Result<MediaState> {
    let Some(s) = session()? else {
        return Ok(MediaState::default());
    };
    session_state(&s)
}

fn session_state(s: &Session) -> Result<MediaState> {
    let props = wait(s.TryGetMediaPropertiesAsync()?, 600)?;
    let info = s.GetPlaybackInfo()?;
    let playing = info.PlaybackStatus()? == Status::Playing;
    let tl = s.GetTimelineProperties()?;

    let end = tl.EndTime()?.Duration - tl.StartTime()?.Duration;
    let mut pos = tl.Position()?.Duration - tl.StartTime()?.Duration;
    // Bazı uygulamalar konumu seyrek günceller; çalıyorsa geçen süreyi ekle.
    if playing {
        let updated = tl.LastUpdatedTime()?.UniversalTime;
        if updated > 0 {
            pos += (now_ticks() - updated).max(0);
        }
    }
    if end > 0 {
        pos = pos.clamp(0, end);
    }

    let title = props.Title()?.to_string();
    let artist = props.Artist()?.to_string();
    let album = props.AlbumTitle().map(|a| a.to_string()).unwrap_or_default();
    Ok(MediaState {
        id: String::new(),
        current: false,
        active: true,
        track_key: format!("{title}\u{1}{artist}\u{1}{album}"),
        title,
        artist,
        album,
        playing,
        position_ms: pos / 10_000,
        duration_ms: end.max(0) / 10_000,
        source: s.SourceAppUserModelId()?.to_string(),
    })
}

/// Çalan parçanın kapak görseli (genelde JPEG/PNG baytları).
pub fn thumbnail() -> Result<Option<Vec<u8>>> {
    let Some(s) = session()? else {
        return Ok(None);
    };
    let props = wait(s.TryGetMediaPropertiesAsync()?, 600)?;
    let Ok(thumb) = props.Thumbnail() else {
        return Ok(None);
    };
    let stream = wait(thumb.OpenReadAsync()?, 1500)?;
    let size = stream.Size()? as u32;
    if size == 0 {
        return Ok(None);
    }
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    wait(reader.LoadAsync(size)?, 1500)?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf)?;
    Ok(Some(buf))
}

pub fn control(action: &str) -> Result<bool> {
    let Some(s) = session()? else {
        return Ok(false);
    };
    let ok = match action {
        // Komutu gönderip beklemeyiz; oynatıcı cevap vermese de dock takılmaz.
        "toggle" => s.TryTogglePlayPauseAsync().is_ok(),
        "next" => s.TrySkipNextAsync().is_ok(),
        "prev" => s.TrySkipPreviousAsync().is_ok(),
        _ => false,
    };
    Ok(ok)
}

/// Tüm medya oturumları (birden çok oynatıcı: Spotify, tarayıcı sekmeleri...).
fn sessions() -> Result<Vec<(String, Session)>> {
    let mgr = manager()?;
    let list = mgr.GetSessions()?;
    let mut out: Vec<(String, Session)> = Vec::new();
    for i in 0..list.Size()? {
        let s = list.GetAt(i)?;
        let app = s.SourceAppUserModelId()?.to_string();
        let n = out.iter().filter(|(id, _)| id.starts_with(&format!("{app}#"))).count();
        out.push((format!("{app}#{n}"), s));
    }
    Ok(out)
}

pub fn states() -> Result<Vec<MediaState>> {
    let current = manager()?
        .GetCurrentSession()
        .ok()
        .and_then(|s| s.SourceAppUserModelId().ok())
        .map(|h| h.to_string());
    let mut out = Vec::new();
    for (id, s) in sessions()? {
        if let Ok(mut st) = session_state(&s) {
            st.current = current.as_deref().map(|c| id.starts_with(&format!("{c}#"))).unwrap_or(false);
            st.id = id;
            out.push(st);
        }
    }
    Ok(out)
}

fn find(id: &str) -> Result<Option<Session>> {
    Ok(sessions()?.into_iter().find(|(i, _)| i == id).map(|(_, s)| s))
}

pub fn control_id(id: &str, action: &str) -> Result<bool> {
    let Some(s) = find(id)? else { return Ok(false) };
    Ok(match action {
        "toggle" => s.TryTogglePlayPauseAsync().is_ok(),
        "next" => s.TrySkipNextAsync().is_ok(),
        "prev" => s.TrySkipPreviousAsync().is_ok(),
        _ => false,
    })
}

pub fn thumbnail_id(id: &str) -> Result<Option<Vec<u8>>> {
    let Some(s) = find(id)? else { return Ok(None) };
    let props = wait(s.TryGetMediaPropertiesAsync()?, 600)?;
    let Ok(thumb) = props.Thumbnail() else { return Ok(None) };
    let stream = wait(thumb.OpenReadAsync()?, 1500)?;
    let size = stream.Size()? as u32;
    if size == 0 {
        return Ok(None);
    }
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    wait(reader.LoadAsync(size)?, 1500)?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf)?;
    Ok(Some(buf))
}
