use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the shared media directory from BOT_MEDIA_DIR env var.
/// This directory must be mounted to the same path in both bot and napcat containers.
pub fn media_dir() -> PathBuf {
    env::var("BOT_MEDIA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/bot-media"))
}

/// Writes bytes to a file in the shared media directory, returns a `file://` URI string
/// that napcat can read directly from its own container.
pub fn write_media(bytes: &[u8], ext: &str) -> Result<String, String> {
    let dir = media_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("创建媒体目录失败：{}", e))?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let filename = format!("bot_media_{}.{}", ts, ext);
    let path = dir.join(&filename);

    fs::write(&path, bytes).map_err(|e| format!("写入媒体文件失败：{}", e))?;

    Ok(format!("file://{}", path.display()))
}
