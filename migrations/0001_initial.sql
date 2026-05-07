CREATE TABLE properties (
    id            BLOB PRIMARY KEY,
    name          TEXT NOT NULL,
    custom_cards  TEXT NOT NULL DEFAULT '[]',
    is_protected  INTEGER NOT NULL DEFAULT 0,
    is_public     INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);

CREATE TABLE events (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    property_id     BLOB NOT NULL REFERENCES properties(id) ON DELETE CASCADE,
    event           TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    user_id         TEXT,
    url             TEXT,
    title           TEXT,
    referrer        TEXT,
    user_agent      TEXT,
    platform        TEXT,
    browser         TEXT,
    device          TEXT,
    screen_width    INTEGER,
    screen_height   INTEGER,
    country         TEXT,
    region          TEXT,
    city            TEXT,
    lat             REAL,
    lon             REAL,
    utm_source      TEXT,
    utm_medium      TEXT,
    utm_campaign    TEXT,
    utm_term        TEXT,
    utm_content     TEXT,
    time_on_page_ms INTEGER,
    extra           TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX events_property_created       ON events(property_id, created_at);
CREATE INDEX events_property_event_created ON events(property_id, event, created_at);

CREATE TABLE bot_events (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    property_id  BLOB NOT NULL REFERENCES properties(id) ON DELETE CASCADE,
    event        TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    bot_name     TEXT,
    url          TEXT,
    user_agent   TEXT,
    country      TEXT,
    extra        TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX bot_events_property_created ON bot_events(property_id, created_at);

-- meta key-value table for things like the Proprium property id
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
