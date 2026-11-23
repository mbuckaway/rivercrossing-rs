// SPDX-License-Identifier: GPL-3.0-only

//! SQLite persistence for `RiverCrossing`: the schema and its frozen
//! migrations, the `Store` over rides, rosters and the event log, the session
//! bookkeeping that the resume dialog reads, and the backup primitives.
//!
//! The crate is UI-agnostic. It owns the one database connection and never
//! reaches above the database: timestamps are UTC instants, and the UI renders
//! local time.

#![forbid(unsafe_code)]

pub mod backup;
pub mod engine_load;
pub mod error;
pub mod migrations;
pub mod paths;
pub mod row;
pub mod schema;
pub mod session;
pub mod store;

pub use error::StoreError;
pub use migrations::{MIGRATIONS, Migration, run_migrations};
pub use paths::default_db_path;
pub use schema::{SCHEMA_VERSION, apply_pragmas, ensure_schema};
