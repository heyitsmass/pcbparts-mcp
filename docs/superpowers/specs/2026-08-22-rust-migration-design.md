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
| `pcbparts-parsers` | `parsers.py`, `pinout.py`, `mounting.py`, `alternatives.py`, `manufacturer_aliases.py`, `subcategory_aliases.py`, `design_rules.py` | Pure logic, no I/O. |
| `pcbparts-search` | `search/` | Depends on `pcbparts-db` + `pcbparts-parsers`. |
| `pcbparts-smart-parser` | `smart_parser/` | Free-text query parsing ("10k 0603 1%" → structured filters). Depends on `pcbparts-parsers` (`subcategory_aliases`) **and** `pcbparts-search` (`SpecFilter` — `smart_parser/parser.py` imports it directly). Cannot live in the `pcbparts-parsers` crate itself: that would make `pcbparts-parsers` depend on `pcbparts-search`, which already depends on `pcbparts-parsers` — a circular crate dependency Cargo rejects outright. Its own crate, built after both. |
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

**Corrected 2026-08-22 during Phase 1 implementation planning**, then
**corrected again 2026-08-22 during Phase 2 implementation planning**, then
**corrected a third time 2026-08-22 during Phase 3 spec design.** All three
corrections came from reading the actual source rather than trusting the
original draft's assumptions:

- The original draft assumed `pcbparts-db` had "no cross-module deps" and
  could go first, but `db/attributes.py`, `db/categories.py`, and
  `db/__init__.py` (whose `ComponentDatabase.search()` delegates entirely to
  `SearchEngine`) all import from `alternatives.py` and `search/*`.
- The Phase 2 draft bundled `smart_parser/*` into `pcbparts-parsers`, but
  `smart_parser/parser.py` imports `SpecFilter` from `search/spec_filter.py`
  directly — `smart_parser` depends on `pcbparts-search`, which itself
  depends on `pcbparts-parsers`. It cannot ship as part of Phase 2; it needs
  its own crate/phase after Phase 3.
- The Phase 2 draft also listed `test_do_now_fixes.py` and
  `test_error_masking.py` as Phase 2 tests. Reading them: neither contains
  any test that exercises Phase 2's modules in isolation. Both are
  cross-cutting regression suites — `test_do_now_fixes.py` touches
  `config.py`/`cache.py` (Phase 9), `client.py` (Phase 7), `server.py`
  (Phase 9), `search/spec_filter.py`'s `SpecFilter` (Phase 3), and
  `scripts/scrape_components.py`/`build_sensor_db.py` (Phase 8);
  `test_error_masking.py` exercises the whole FastMCP server object (Phase
  9) via `fastmcp.Client`. Removed from Phase 2's list; each becomes test
  material for whichever later phase owns the function it exercises.
- The original draft listed `test_distributors.py` and
  `boards_eval/test_search_quality.py` as Phase 3 test material. Reading
  them: `test_distributors.py` imports only from `mouser.py`, `digikey.py`,
  `cse.py`, and `cache.py` — Phase 6 (`pcbparts-clients`) and Phase 9
  (`pcbparts-server`), nothing from `search/`. `test_search_quality.py`
  imports only `pcbparts_mcp.boards_db` — Phase 1, already done, and
  unrelated to component search. Both removed from Phase 3's list.
  `test_resolvers.py` (kept) actually imports only from `search/mpn.py`
  (`normalize_mpn`, `looks_like_mpn`) despite its name — real Phase 3
  material, just misattributed to `resolvers.py`'s own functions
  (`expand_query_synonyms`, `expand_package`, `resolve_manufacturer`),
  which have no dedicated test file. Net effect: of `search/`'s ~1,660
  lines across `engine.py`, `query_builder.py`, `spec_filter.py`,
  `result.py`, and `resolvers.py`'s real functions, **only `mpn.py` has
  existing pytest coverage** — everything else needs new characterization
  tests generated during implementation (see Phase 3 notes below).
- `search/engine.py` imports `DEFAULT_MIN_STOCK` from `config.py` (Phase
  9), used only as `search()`'s default-argument value
  (`min_stock: int = DEFAULT_MIN_STOCK`). Resolution: Rust has no default
  arguments, so `pcbparts-search`'s `search()` takes `min_stock: i64` as a
  required parameter — applying "10 if the caller didn't specify" becomes
  Phase 9's job when it wires the server together. No `config.py`/Phase 9
  dependency needed in Phase 3.

The real dependency graph, from the imports as they exist today:

- `parsers.py`, `mounting.py`, `pinout.py`, and `design_rules.py` are
  genuinely independent (stdlib-only regex and string logic).
- `alternatives.py` depends only on `parsers.py`.
- `manufacturer_aliases.py` and `subcategory_aliases.py` are independent data
  tables.
- Every file in `search/` depends on some combination of `alternatives.py`,
  `parsers.py`, `mounting.py`, `subcategory_aliases.py`,
  `manufacturer_aliases.py`, and `config.py` (the last is only
  `DEFAULT_MIN_STOCK`, a default-argument value — resolved as a required
  parameter in Rust rather than a real Phase 3 crate dependency; see the
  Phase 3 correction above).
- `smart_parser/parser.py` depends on `search/spec_filter.py` (`SpecFilter`)
  and `smart_parser/types.py` depends on `subcategory_aliases.py` — so all of
  `smart_parser/` depends on both `pcbparts-parsers` and `pcbparts-search`.
- The component-DB half of `db/` (`db/__init__.py`, `attributes.py`,
  `categories.py`, `lookup.py`) depends on all of the above plus `search/`
  (not on `smart_parser` — only `server.py` imports that, at the tool layer).
- `boards_db/` and `sensor_db/` are fully self-contained — no imports from
  `search/`, `alternatives.py`, `parsers.py`, or `config.py`.

Each phase ships when its crate's Rust tests pass. Later phases depend on
earlier ones being in place.

1. **`pcbparts-db` (boards + sensor half only)** — `boards_db/`, `sensor_db/`.
   Genuinely zero cross-module deps; the first real phase. **Done** — see
   `docs/superpowers/plans/2026-08-22-rust-phase1-db-crate.md`.
   Tests: `test_boards_db.py`, `test_sensor_db.py`.
2. **`pcbparts-parsers`** — `parsers.py`, `mounting.py`, `alternatives.py`,
   `manufacturer_aliases.py`, `subcategory_aliases.py`, `pinout.py`,
   `design_rules.py`. No `smart_parser` here (see above). Large enough
   (~3,400 source lines, `alternatives.py` alone is 1,573 with its
   `COMPATIBILITY_RULES` data table) to split into two implementation
   plans: **2A** (`parsers.py`, `mounting.py`, `manufacturer_aliases.py`,
   `subcategory_aliases.py`, `pinout.py`, `design_rules.py`) and **2B**
   (`alternatives.py`, which depends on 2A's `parsers.rs`).
   Tests: `test_parsers.py` (the parsers.py-testing portion only — the file
   also inline-imports from `smart_parser/*` for its back half, which is
   Phase 4's test material, not Phase 2's), `test_mounting.py`,
   `test_alternatives.py` (everything except the
   `@pytest.mark.integration class TestFindAlternativesIntegration` at the
   end, which needs the wafer bridge — Phase 7), `test_pinout.py` (the
   `TestParsePins` class only — `TestPinoutIntegration` needs Phase 7),
   `test_design_rules.py`. `manufacturer_aliases.py` and
   `subcategory_aliases.py` have no dedicated test files today — ported
   as data + the functions in `subcategory_aliases.py`
   (`resolve_subcategory_name`, `find_similar_subcategories`), with no new
   tests invented, matching current coverage.
3. **`pcbparts-search`** — `search/mpn.py`, `spec_filter.py`, `resolvers.py`,
   `result.py`, `query_builder.py`, `engine.py`. Depends on phase 2 and on
   `pcbparts-db`'s `Connection` type from phase 1 (not on phase 5's
   component-DB code — see below). One Rust module per Python file, same
   as Phase 2's convention. **Done** — see
   `docs/superpowers/plans/2026-08-22-rust-phase3-search-crate.md`
   (main `f40fd80`, 323/323 workspace tests passing).
   Tests: `test_resolvers.py` (covers `mpn.py` only). No other dedicated
   pytest file exists for this crate's ~1,660 remaining lines (see the
   Phase 3 correction above) — coverage comes from new characterization
   tests generated during implementation, following the same "golden
   values, not re-derived" principle as every other phase, just generated
   rather than pre-existing:
   - Build `data/components.db` locally (git-ignored, not checked in) via
     `docker run --rm -v "$(pwd)":/workspace -w /workspace python:3.12-slim
     python scripts/build_database.py --data-dir data --output
     data/components.db` — the script is stdlib-only (plus this repo's
     own `pcbparts_mcp.parsers`), so no pip install is needed inside the
     container. As of this design pass: 618,277 parts, 843 subcategories,
     55 categories, ~643MB.
   - A Python script (written during implementation, mirroring Phase 1's
     `dump_boards_fixture.py`) instantiates the existing Python
     `ComponentDatabase` against that DB and runs a curated set of
     representative `search()` calls — plain FTS query, subcategory by
     name/id, category filters, `spec_filters` across different parser
     types, `package`/`packages`, `manufacturer` (with alias resolution),
     `mounting_type`, each `sort_by` variant, pagination, and edge cases
     (subcategory-not-found → similar-suggestions, zero results, an
     MPN-shaped query) — dumping each call's actual JSON output as a
     golden fixture the Rust port is tested against.
   - The four smaller pure-function files (`mpn.py` already has real
     tests; `resolvers.py`, `spec_filter.py`, `query_builder.py`,
     `result.py` do not) get the same treatment at the unit level: call
     directly with representative inputs, capture actual output as
     fixtures.
   - `SearchEngine`'s constructor (`conn: sqlite3.Connection`,
     `subcategories`/`categories`/`*_name_to_id` maps) is ported as plain
     parameters — Phase 3 doesn't depend on phase 5's cache-building code
     to define or test the engine, only on the shape of the maps
     themselves (built by test fixtures/characterization data here, and
     by phase 5's real cache loader once that phase exists).
   - `DEFAULT_MIN_STOCK` (`config.py`, phase 9): not a Phase 3 dependency
     — see the correction above. `search()` takes `min_stock: i64` as a
     required parameter.
   - `result.py`'s `row_to_dict` builds its `specs` map from
     `{name: value for name, value in attrs}`, where `attrs` is a JSON
     array of `[name, value]` pairs stored per-row — a Python dict
     comprehension over an ordered list preserves that order. This is the
     same order-sensitivity Phase 2B found (and fixed by enabling
     `serde_json`'s `preserve_order` feature) in `alternatives.rs`'s
     `specs_to_verify`. `pcbparts-search`'s `Cargo.toml` must explicitly
     declare `preserve_order` on its own `serde_json` dependency too —
     don't rely on Cargo's workspace feature unification silently
     supplying it (that exact latent, build-scope-dependent trap was
     flagged and rejected as a fix in Phase 2B's final review).
4. **`pcbparts-smart-parser`** — `smart_parser/*`. Depends on phases 2 + 3.
   Tests: the `smart_parser`-testing portion of `test_parsers.py` (the
   inline `from pcbparts_mcp.smart_parser.X import Y` tests), plus the
   `smart_parser`-integration-through-the-DB tests in `test_db.py` — those
   specifically need phase 5 (component db) too, so they're the one test
   file split across two phases; port what's portable here, finish the rest
   in phase 5.
5. **`pcbparts-db` (component half)** — `db/__init__.py`, `attributes.py`,
   `categories.py`, `lookup.py`, `connection.py`, `stats.py`. Depends on
   phases 2 + 3 (not 4 — `db/` itself never imports `smart_parser`).
   Completes the `pcbparts-db` crate started in phase 1.
   Tests: `test_db.py` (its `smart_parser`-integration cases depend on phase
   4 as well — see above).
6. **`pcbparts-clients`** — independent of the above; no wafer dependency,
   so no ordering constraint relative to the bridge.
7. **`pcbparts-wafer-bridge`** — wraps `client.py` as-is.
   Tests: `test_client.py` (integration-marked) becomes a thin Rust test
   calling through the bridge into the unmodified Python client.
8. **`pcbparts-pipeline`** — scraper orchestration/parsing in Rust; fetch
   calls go through the bridge from phase 7.
   Tests: `test_build_database.py`, `test_parse_boards.py`,
   `test_scrape_status.py`, `test_sensor_merge.py`,
   `test_sensor_recommend.py`, `test_history.py`, `test_kicad_legacy.py`,
   `boards_eval/`.
9. **`pcbparts-server`** — last, since it wires every other crate together
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

**When no Python test exists for a function** (first seen in Phase 2B's
`build_response`/`build_unsupported_response`, formalized here because
Phase 3 hits it at much larger scale): the default is a compile-level
smoke test only, matching current (zero) coverage exactly — no test
invented beyond what exists. Where the function is significant enough
that shipping it with zero behavioral verification is a real risk (all of
`pcbparts-search` except `mpn.py`, per the Phase 3 correction above), the
alternative is a **characterization test**: run the existing Python
function directly against representative inputs, capture its actual
output as a new golden fixture, and port against that fixture instead of
against a pre-existing pytest assertion. This is still "golden values, not
re-derived" — the golden value is just generated during implementation
rather than already sitting in the test suite.

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
| `smart_parser/*.py` | `pcbparts-smart-parser` |

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
