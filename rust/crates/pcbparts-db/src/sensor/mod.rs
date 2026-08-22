//! Sensor database: schema + module wiring.
pub mod search;

pub const SCHEMA: &str = "
CREATE TABLE sensors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    manufacturer TEXT,
    type TEXT,
    voltage TEXT,
    datasheet_url TEXT,
    platform_count INTEGER DEFAULT 0,
    description TEXT,
    source_tier TEXT DEFAULT 'primary',
    sources TEXT
);
CREATE TABLE sensor_measures (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    measure TEXT NOT NULL,
    PRIMARY KEY (sensor_id, measure)
);
CREATE TABLE sensor_protocols (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    protocol TEXT NOT NULL,
    PRIMARY KEY (sensor_id, protocol)
);
CREATE TABLE sensor_platforms (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    platform TEXT NOT NULL,
    PRIMARY KEY (sensor_id, platform)
);
CREATE TABLE sensor_urls (
    sensor_id TEXT NOT NULL REFERENCES sensors(id),
    url TEXT NOT NULL,
    PRIMARY KEY (sensor_id, url)
);
CREATE VIRTUAL TABLE sensors_fts USING fts5(id, name, manufacturer, description);
";
