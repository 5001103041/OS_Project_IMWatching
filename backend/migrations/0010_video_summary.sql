-- Migration 0010: cache AI-generated video summary on the videos row.
-- Generated once when the chatbot first needs it (or on demand via the
-- summary endpoint), invalidated whenever the video is reprocessed.

ALTER TABLE videos ADD COLUMN ai_summary TEXT;
ALTER TABLE videos ADD COLUMN ai_summary_generated_at TEXT;
