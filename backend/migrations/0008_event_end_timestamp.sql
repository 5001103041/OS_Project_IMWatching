-- Add end timestamp so each detected behavior is a time RANGE (start→end),
-- not just a single instant. end == start when unknown.
ALTER TABLE detected_events ADD COLUMN end_timestamp_secs REAL;
CREATE INDEX IF NOT EXISTS idx_events_end_ts ON detected_events(video_id, end_timestamp_secs);
