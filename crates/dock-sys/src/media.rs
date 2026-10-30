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

#[derive(Debug, Clone, Serialize, Default)]
pub struct MediaState {
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

fn session() -> Result<Option<Session>> {
    let mgr = Manager::RequestAsync()?.join()?;
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
    let props = s.TryGetMediaPropertiesAsync()?.join()?;
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
    let props = s.TryGetMediaPropertiesAsync()?.join()?;
    let Ok(thumb) = props.Thumbnail() else {
        return Ok(None);
    };
    let stream = thumb.OpenReadAsync()?.join()?;
    let size = stream.Size()? as u32;
    if size == 0 {
        return Ok(None);
    }
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    reader.LoadAsync(size)?.join()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf)?;
    Ok(Some(buf))
}

pub fn control(action: &str) -> Result<bool> {
    let Some(s) = session()? else {
        return Ok(false);
    };
    let ok = match action {
        "toggle" => s.TryTogglePlayPauseAsync()?.join()?,
        "next" => s.TrySkipNextAsync()?.join()?,
        "prev" => s.TrySkipPreviousAsync()?.join()?,
        _ => false,
    };
    Ok(ok)
}
