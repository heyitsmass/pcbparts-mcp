# pcbparts-mcp: Python → Rust Migration Design

**Date:** 2026-08-22
**Status:** Approved for planning

## Goal

Rewrite pcbparts-mcp entirely in Rust. The primary success criterion is
behavioral parity: every test that currently exists in the Python test suite
has a passing equivalent after migration. This is a rewrite, not an
incremental evolution of the Python service — CLAUDE.md's production
breaking-change/semver caution does not gate this work (see project memory
`project-rust-rewrite`). Git commit rules, the wafer usage rule, and "never
test via the live MCP" still apply throughout.

## Scope

"Entire repo" — both the runtime MCP server (`src/pcbparts_mcp/`) and the
offline data pipeline (`scripts/`) move to Rust, with one deliberate,
permanent exception: everywhere the code calls into `wafer` (anti-detection
HTTP with TLS fingerprinting / WAF solving) stays as unmodified Python,
invoked from Rust through a PyO3 embedding bridge.

This exception is not a stalled corner of the migration to revisit later —
it is a standing architectural boundary. `wafer`'s anti-detection behavior
has no Rust equivalent, reimplementing it is an unproven, adversarial
problem (JLCPCB's WAF already 403'd a scrape once over a subtler behavior
change — see CLAUDE.md's `timeout`/`attempt_timeout` pairing gotcha), and
nothing about "migrate to Rust" requires re-litigating that fight. A future,
fully-native replacement (e.g. a Rust TLS-impersonation crate) is explicitly
**out of scope** for this migration.

Two modules call `wafer` directly today:
- `src/pcbparts_mcp/client.py` (`JLCPCBClient`) — live JLCPCB search/stock/
  part-detail calls and EasyEDA symbol fetch, used by 5 of the 14 MCP tools
  (`jlc_stock_check`, `jlc_search`, `jlc_get_part`, `jlc_find_alternatives`,
  `jlc_get_pinout`).
- `scripts/scrapers/common.py` (`make_session`/`attempt_cap`) — the fetch
  layer shared by all 14 offline sensor scrapers.

Everything else in both the runtime server and the offline pipeline —
SQLite access, all parsing/business logic, the Mouser/DigiKey/CSE HTTP
clients (plain `httpx` against non-adversarial official APIs, no bridge
needed), the MCP tool surface itself, and all scraper orchestration/HTML
cleanup/classification logic — becomes native Rust.

## Target Architecture

A Cargo workspace with crates mirroring current module boundaries:

| Crate | Replaces | Notes |
|---|---|---|
| `pcbparts-db` | `db/`, `boards_db/`, `sensor_db/` | `rusqlite`. DB file schemas are the stable contract — unchanged, so the Rust reader is drop-in compatible with today's Python-built DBs throughout the transition. |
| `pcbparts-parsers` | `parsers.py`, `smart_parser/`, `pinout.py`, `mounting.py`, `alternatives.py`, `manufacturer_aliases.py`, `subcategory_aliases.py`, `design_rules.py` | Pure logic, no I/O. |
| `pcbparts-search` | `search/` | Depends on `pcbparts-db` + `pcbparts-parsers`. |
| `pcbparts-clients` | `mouser.py`, `digikey.py`, `cse.py` | `reqwest`. No anti-bot concerns — official APIs / plain HTTP. |
| `pcbparts-wafer-bridge` | the `wafer`-calling parts of `client.py` and `scripts/scrapers/common.py` | PyO3 embedding crate. Owns every `Python::with_gil` call into unmodified Python. Named for what it bridges (wafer), not which caller uses it, since both the JLCPCB client and the sensor scrapers depend on it. |
| `pcbparts-server` | `server.py`, `cache.py`, `config.py` | `rmcp` + `tokio` + `axum`, replacing `fastmcp` + `uvicorn` + `starlette`. Wires every other crate into the 14 MCP tools. Ported last. |
| `pcbparts-pipeline` | `scripts/` (minus the wafer fetch calls) | Scraper orchestration, HTML cleanup, sensor classification, board/footprint parsing (`parsers/eagle.py`, `kicad.py`, `kicad_legacy.py`). Fetch calls go through `pcbparts-wafer-bridge`. Produces the same DB files `pcbparts-db` reads — decoupled from the runtime server, runs on the existing GitHub Actions schedule. |

### PyO3 bridge concurrency

Only 5 of 14 runtime tools touch the bridge, but PyO3 embedding serializes
Python calls under the GIL. The bridge crate must isolate its calls behind a
bounded worker pool (a small number of dedicated OS threads each holding the
GIL for one call at a time) so wafer-bound requests don't stall the Tokio
runtime for the other 9 tools. This is an implementation detail of
`pcbparts-wafer-bridge`, not a reason to avoid the bridge.

### Deployment implication

The Docker image must still ship a Python 3.12 runtime plus `wafer-py` for
the bridge crate to link against and call into. "Entire repo in Rust" means
the code that isn't the wafer boundary, not zero Python in the production
image.

## Migration Order

**Corrected 2026-08-22 during implementation planning.** The order below was
revised after reading the actual source: the original draft assumed
`pcbparts-db` had "no cross-module deps" and could go first, but
`db/attributes.py`, `db/categories.py`, and `db/__init__.py` (whose
`ComponentDatabase.search()` delegates entirely to `SearchEngine`) all
import from `alternatives.py` and `search/*`. The real dependency graph, from
the imports as they exist today:

- `parsers.py` and `mounting.py` are genuinely independent (stdlib-only regex
  and string logic).
- `alternatives.py` depends only on `parsers.py`.
- `manufacturer_aliases.py` and `subcategory_aliases.py` are independent data
  tables.
- Every file in `search/` depends on some combination of `alternatives.py`,
  `parsers.py`, `mounting.py`, `subcategory_aliases.py`,
  `manufacturer_aliases.py`, and `config.py`.
- The component-DB half of `db/` (`db/__init__.py`, `attributes.py`,
  `categories.py`, `lookup.py`) depends on all of the above plus `search/`.
- `boards_db/` and `sensor_db/` are fully self-contained — no imports from
  `search/`, `alternatives.py`, `parsers.py`, or `config.py`.

Each phase ships when its crate's Rust tests pass. Later phases depend on
earlier ones being in place.

1. **`pcbparts-db` (boards + sensor half only)** — `boards_db/`, `sensor_db/`.
   Genuinely zero cross-module deps; the first real phase.
   Tests: `test_boards_db.py`, `test_sensor_db.py`.
2. **`pcbparts-parsers`** — `parsers.py`, `mounting.py`, `alternatives.py`,
   `manufacturer_aliases.py`, `subcategory_aliases.py`, plus the
   independent-of-search-and-db pieces: `pinout.py`, `design_rules.py`,
   `smart_parser/*`.
   Tests: `test_parsers.py`, `test_mounting.py`, `test_alternatives.py`,
   `test_pinout.py`, `test_design_rules.py`, `test_do_now_fixes.py`,
   `test_error_masking.py`.
3. **`pcbparts-search`** — `search/mpn.py`, `spec_filter.py`, `resolvers.py`,
   `result.py`, `query_builder.py`, `engine.py`. Depends on phase 2.
   Tests: `test_resolvers.py`, `test_distributors.py`, search-relevant parts
   of `boards_eval/` (`test_search_quality.py`).
4. **`pcbparts-db` (component half)** — `db/__init__.py`, `attributes.py`,
   `categories.py`, `lookup.py`, `connection.py`, `stats.py`. Depends on
   phases 2 + 3. Completes the `pcbparts-db` crate started in phase 1.
   Tests: `test_db.py`.
5. **`pcbparts-clients`** — independent of the above; no wafer dependency,
   so no ordering constraint relative to the bridge.
6. **`pcbparts-wafer-bridge`** — wraps `client.py` as-is.
   Tests: `test_client.py` (integration-marked) becomes a thin Rust test
   calling through the bridge into the unmodified Python client.
7. **`pcbparts-pipeline`** — scraper orchestration/parsing in Rust; fetch
   calls go through the bridge from phase 6.
   Tests: `test_build_database.py`, `test_parse_boards.py`,
   `test_scrape_status.py`, `test_sensor_merge.py`,
   `test_sensor_recommend.py`, `test_history.py`, `test_kicad_legacy.py`,
   `boards_eval/`.
8. **`pcbparts-server`** — last, since it wires every other crate together
   into the 14 MCP tools. Cutover happens here.

## Test-Parity Mechanics

Every ported module gets its pytest file translated to an equivalent Rust
`#[test]`/`#[tokio::test]` module, asserting the same inputs → outputs.
Expected values are golden values pulled from current Python behavior, not
re-derived from scratch — the Rust test's job is to prove the port matches
existing behavior, not to re-specify it.

Until a module is ported, its existing Python tests keep running unchanged
as the live parity baseline — CI runs both suites side by side for the
duration of the migration.

"Passing every test that currently exists in Python" is complete when all
~20 test files have a passing Rust counterpart. Tests that exercise the
wafer-bridged path (`test_client.py`, the scraper-fetch pieces of the
sensor scraper tests) count as passing because they call the unchanged,
already-passing Python code through the bridge — they are not required to
become pure-Rust assertions with no Python involved.

## Risks / Non-Goals

- **GIL serialization** — mitigated by the bounded worker pool in
  `pcbparts-wafer-bridge` (see above); only 5 of 14 tools affected.
- **Two-runtime deployment** — the production image carries both a Rust
  binary and a Python environment; this is accepted, not a defect to fix.
- **Non-goal:** replacing `wafer` with a native-Rust anti-detection client.
  Explicitly future work, not part of this migration, and not a blocker for
  calling the migration complete.
- **Still-applicable CLAUDE.md rules during this work:** never commit
  without explicit permission, no Claude attribution in commits, always use
  `wafer` (via the bridge) for JLCPCB/EasyEDA/sensor-scraper calls — never
  raw httpx/aiohttp against those hosts, never test changes via the live
  MCP (local DB + local server only), pair `timeout=`/`attempt_timeout=`
  wherever wafer sessions are constructed.
- **Waived for this work specifically:** the "Beta with real external
  users — breaking changes must be deliberate" caution and the exact
  dependency-pin discipline in `pyproject.toml`, per the `project-rust-rewrite`
  memory. This is scoped to rewrite planning/design — if asked to touch the
  live Python service directly (not rewrite work), confirm the scoping
  still holds.

## Appendix: Full File → Crate Mapping

### Runtime server (`src/pcbparts_mcp/`)

| File | Crate |
|---|---|
| `alternatives.py` | `pcbparts-parsers` |
| `boards_db/*.py` | `pcbparts-db` |
| `cache.py` | `pcbparts-server` |
| `client.py` | `pcbparts-wafer-bridge` |
| `config.py` | `pcbparts-server` |
| `cse.py` | `pcbparts-clients` |
| `db/*.py` | `pcbparts-db` |
| `design_rules.py` | `pcbparts-parsers` |
| `digikey.py` | `pcbparts-clients` |
| `manufacturer_aliases.py`, `subcategory_aliases.py` | `pcbparts-parsers` |
| `mounting.py` | `pcbparts-parsers` |
| `mouser.py` | `pcbparts-clients` |
| `parsers.py` | `pcbparts-parsers` |
| `pinout.py` | `pcbparts-parsers` |
| `search/*.py` | `pcbparts-search` |
| `sensor_db/*.py` | `pcbparts-db` |
| `server.py` | `pcbparts-server` |
| `smart_parser/*.py` | `pcbparts-parsers` |

### Offline pipeline (`scripts/`)

| File | Crate |
|---|---|
| `board_overrides.py`, `build_boards_db.py`, `build_database.py`, `build_history_db.py`, `build_sensor_db.py`, `check_scrape_status.py`, `cleanup_html.py`, `extract_source.py`, `parse_boards.py`, `scrape_components.py`, `scrape_sensors.py`, `strip_notices.py`, `verify_scrape_feasibility.py` | `pcbparts-pipeline` |
| `parsers/{__init__,common,eagle,kicad,kicad_legacy}.py` | `pcbparts-pipeline` |
| `scrapers/{__init__,arduino,atlas_scientific,benewake,bestmodules,circuitpython,dfrobot,esphome,hilink,maxbotix,micropython,sparkfun,tasmota,winsen,zephyr}.py` | `pcbparts-pipeline` (orchestration/parsing) + `pcbparts-wafer-bridge` (fetch calls) |
| `scrapers/common.py` | split: pure helpers (`extract_manufacturer`, `infer_measures`, `normalize_sensor_id`, etc.) → `pcbparts-pipeline`; `make_session`/`attempt_cap` wafer fetch → `pcbparts-wafer-bridge` |

## Next Step

Hand this spec to the writing-plans skill to produce a phased
implementation plan following the migration order above.
