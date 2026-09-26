-- Detected behavior events (evidence)
CREATE TABLE IF NOT EXISTS detected_events (
    id               TEXT PRIMARY KEY,
    video_id         TEXT NOT NULL,
    frame_id         TEXT,
    timestamp_secs   REAL NOT NULL,
    behavior_name    TEXT NOT NULL,
    severity         TEXT NOT NULL DEFAULT 'normal',      -- 'normal' | 'dangerous'
    gender           TEXT NOT NULL DEFAULT 'unknown',     -- 'male' | 'female' | 'unknown'
    clothing_color   TEXT,
    confidence       REAL NOT NULL DEFAULT 0.0,
    ai_reason        TEXT,
    frame_image_path TEXT NOT NULL,
    review_status    TEXT NOT NULL DEFAULT 'unreviewed',  -- 'unreviewed'|'confirmed'|'false_positive'
    reviewed_by      TEXT,
    reviewed_at      TEXT,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (video_id) REFERENCES videos(id) ON DELETE CASCADE,
    FOREIGN KEY (frame_id) REFERENCES video_frames(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_events_video     ON detected_events(video_id);
CREATE INDEX IF NOT EXISTS idx_events_behavior  ON detected_events(behavior_name);
CREATE INDEX IF NOT EXISTS idx_events_severity  ON detected_events(severity);
CREATE INDEX IF NOT EXISTS idx_events_gender    ON detected_events(gender);
CREATE INDEX IF NOT EXISTS idx_events_color     ON detected_events(clothing_color);
CREATE INDEX IF NOT EXISTS idx_events_ts        ON detected_events(video_id, timestamp_secs);
