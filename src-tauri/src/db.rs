use rusqlite::{Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A download history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadHistoryEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub artist: String,
    pub thumbnail: Option<String>,
    pub duration: Option<u64>,
    #[serde(rename = "outputPath")]
    pub output_path: String,
    #[serde(rename = "downloadedAt")]
    pub downloaded_at: String,
}

/// Get the path to the database file
fn get_db_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sample-downloader")
        .join("history.db")
}

/// Initialize the database and create tables if needed
pub fn init_db() -> SqlResult<Connection> {
    let db_path = get_db_path();

    // Create parent directory if needed
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = Connection::open(&db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS download_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            thumbnail TEXT,
            duration INTEGER,
            output_path TEXT NOT NULL,
            downloaded_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    // Create index for faster searching
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_history_downloaded_at ON download_history(downloaded_at DESC)",
        [],
    )?;

    Ok(conn)
}

/// Save a download to history
pub fn save_download(
    url: &str,
    title: &str,
    artist: &str,
    thumbnail: Option<&str>,
    duration: Option<u64>,
    output_path: &str,
) -> SqlResult<i64> {
    let conn = init_db()?;

    conn.execute(
        "INSERT INTO download_history (url, title, artist, thumbnail, duration, output_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![url, title, artist, thumbnail, duration.map(|d| d as i64), output_path],
    )?;

    Ok(conn.last_insert_rowid())
}

/// Get recent downloads
pub fn get_history(limit: u32) -> SqlResult<Vec<DownloadHistoryEntry>> {
    let conn = init_db()?;

    let mut stmt = conn.prepare(
        "SELECT id, url, title, artist, thumbnail, duration, output_path, downloaded_at
         FROM download_history
         ORDER BY downloaded_at DESC
         LIMIT ?1",
    )?;

    let entries = stmt
        .query_map([limit], |row| {
            Ok(DownloadHistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                artist: row.get(3)?,
                thumbnail: row.get(4)?,
                duration: row.get::<_, Option<i64>>(5)?.map(|d| d as u64),
                output_path: row.get(6)?,
                downloaded_at: row.get(7)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(entries)
}

/// Search history by title or artist
pub fn search_history(query: &str, limit: u32) -> SqlResult<Vec<DownloadHistoryEntry>> {
    let conn = init_db()?;

    let search_pattern = format!("%{}%", query);

    let mut stmt = conn.prepare(
        "SELECT id, url, title, artist, thumbnail, duration, output_path, downloaded_at
         FROM download_history
         WHERE title LIKE ?1 OR artist LIKE ?1
         ORDER BY downloaded_at DESC
         LIMIT ?2",
    )?;

    let entries = stmt
        .query_map(rusqlite::params![search_pattern, limit], |row| {
            Ok(DownloadHistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                artist: row.get(3)?,
                thumbnail: row.get(4)?,
                duration: row.get::<_, Option<i64>>(5)?.map(|d| d as u64),
                output_path: row.get(6)?,
                downloaded_at: row.get(7)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(entries)
}

/// Delete a history entry
pub fn delete_history_entry(id: i64) -> SqlResult<()> {
    let conn = init_db()?;
    conn.execute("DELETE FROM download_history WHERE id = ?1", [id])?;
    Ok(())
}

/// Clear all history
pub fn clear_history() -> SqlResult<()> {
    let conn = init_db()?;
    conn.execute("DELETE FROM download_history", [])?;
    Ok(())
}
