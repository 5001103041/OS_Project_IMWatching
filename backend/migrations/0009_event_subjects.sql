-- Store the full list of detected subjects (JSON) so the UI can show
-- multi-person events clearly, not just the first subject.
ALTER TABLE detected_events ADD COLUMN subjects_json TEXT;
