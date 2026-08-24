//! Shared test fixture: points at the real, locally-built components.db —
//! test_db.py's 89 assertions are against specific real parts and real
//! attribute distributions, so there is no small synthetic fixture that can
//! satisfy them. Rebuild via `scripts/build_database.py` if stale (see this
//! plan's Global Constraints).
use std::path::PathBuf;

pub(crate) fn real_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/components.db")
}
