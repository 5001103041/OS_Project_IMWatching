-- Videos (uploaded recordings)
CREATE TABLE IF NOT EXISTS videos (
    id                TEXT PRIMARY KEY,
    owner_id          TEXT NOT NULL,
    title             TEXT,
    original_filename TEXT NOT NULL,
    final_filename    TEXT,
    behavior_category TEXT,
    storage_path      TEXT NOT NULL,
    mime_type         TEXT NOT NULL DEFAULT 'video/mp4',
    duration_secs     REAL,
    width             INTEGER,
    height            INTEGER,
    fps               REAL,
    file_size_bytes   INTEGER,
    status            TEXT NOT NULL DEFAULT 'uploaded',
    error_message     TEXT,
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (owner_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_videos_owner ON videos(owner_id);
CREATE INDEX IF NOT EXISTS idx_videos_status ON videos(status);
