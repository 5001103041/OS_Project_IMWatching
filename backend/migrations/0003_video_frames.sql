-- Extracted frames (1 fps by default)
CREATE TABLE IF NOT EXISTS video_frames (
    id             TEXT PRIMARY KEY,
    video_id       TEXT NOT NULL,
    frame_index    INTEGER NOT NULL,
    timestamp_secs REAL NOT NULL,
    storage_path   TEXT NOT NULL,
    width          INTEGER,
    height         INTEGER,
    analyzed       INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (video_id) REFERENCES videos(id) ON DELETE CASCADE,
    UNIQUE (video_id, frame_index)
);

CREATE INDEX IF NOT EXISTS idx_frames_video ON video_frames(video_id, frame_index);
