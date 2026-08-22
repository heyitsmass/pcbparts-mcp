//! Boards database: schema + module wiring.
pub mod search;
pub mod detail;

#[cfg(test)]
pub(crate) mod fixtures;

pub const SCHEMA: &str = "
CREATE TABLE boards (
    id INTEGER PRIMARY KEY,
    slug TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    org TEXT,
    org_display TEXT,
    source TEXT,
    format TEXT,
    description TEXT,
    key_coverage TEXT,
    layers INTEGER,
    width_mm REAL,
    height_mm REAL,
    min_trace TEXT,
    min_clearance TEXT,
    min_drill TEXT,
    min_via TEXT,
    component_count INTEGER,
    ic_count INTEGER,
    net_count INTEGER,
    key_ics_text TEXT,
    all_ics_text TEXT,
    nets_json TEXT,
    positions_json TEXT,
    copper_pours_json TEXT,
    neighborhoods_json TEXT
);
CREATE TABLE board_tags (
    board_id INTEGER NOT NULL REFERENCES boards(id),
    tag TEXT NOT NULL,
    PRIMARY KEY (board_id, tag)
);
CREATE TABLE board_key_ics (
    board_id INTEGER NOT NULL REFERENCES boards(id),
    ic TEXT NOT NULL,
    PRIMARY KEY (board_id, ic)
);
CREATE TABLE board_components (
    id INTEGER PRIMARY KEY,
    board_id INTEGER NOT NULL REFERENCES boards(id),
    ref TEXT NOT NULL,
    value TEXT,
    footprint TEXT,
    description TEXT,
    voltage TEXT,
    tolerance TEXT,
    dielectric TEXT,
    decouples TEXT,
    pullup TEXT,
    pulldown TEXT
);
CREATE INDEX idx_board_org ON boards(org);
CREATE INDEX idx_board_layers ON boards(layers);
CREATE INDEX idx_board_format ON boards(format);
CREATE INDEX idx_board_component_count ON boards(component_count DESC);
CREATE INDEX idx_board_tag ON board_tags(tag);
CREATE INDEX idx_board_key_ic ON board_key_ics(ic);
CREATE INDEX idx_comp_board_id ON board_components(board_id);
CREATE INDEX idx_comp_value ON board_components(value);
CREATE VIRTUAL TABLE boards_fts USING fts5(
    slug, name, description, key_coverage, tags_text, key_ics_text, all_ics_text, org_display,
    tokenize='porter unicode61'
);
";
