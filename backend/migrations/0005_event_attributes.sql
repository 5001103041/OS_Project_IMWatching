-- Extended event attributes (EAV)
CREATE TABLE IF NOT EXISTS event_attributes (
    id              TEXT PRIMARY KEY,
    event_id        TEXT NOT NULL,
    attribute_type  TEXT NOT NULL,
    attribute_value TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.0,
    FOREIGN KEY (event_id) REFERENCES detected_events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_attrs_event ON event_attributes(event_id);
CREATE INDEX IF NOT EXISTS idx_attrs_type_value ON event_attributes(attribute_type, attribute_value);
