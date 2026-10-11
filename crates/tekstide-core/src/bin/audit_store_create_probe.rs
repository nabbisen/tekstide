//! RFC-059 PR-059-A, D6: two real OS processes creating the audit store for
//! the first time, reproducing the race `rfcs/future-work.md` already
//! records (found at RFC-058 review 515): `AuditStore::open_internal` reads
//! `database_file().exists()` before either connection opens, both see
//! `false`, both reach `create_current_schema`, and `CREATE_SCHEMA_V3` had
//! no `IF NOT EXISTS` -- the loser's own `CREATE TABLE` failed outright.
//!
//! **This is a test-and-proof artifact, not a product feature**, the same
//! shape `project_process_probe` already established for RFC-058's own
//! real-process reproduction. It calls only `tekstide_core::audit`'s real,
//! public `AuditPathResolver`/`AuditStore::open` -- never a reimplementation
//! of either.
//!
//! # Usage
//!
//! ```text
//! audit_store_create_probe <state-root> <project-root>
//! ```
//!
//! Prints `READY` and flushes, then blocks reading one line from stdin --
//! the test harness's own "go" signal, written to both processes' stdin as
//! close together as it can manage so the two genuinely overlap rather than
//! merely running one after the other (the RFC-058 review 513 lesson, named
//! in this RFC's own task breakdown so it does not need re-finding). After
//! release, calls the real `AuditStore::open` and prints exactly one line:
//! `OK` or `ERR <reason>`.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use tekstide_core::audit::{AuditPathRequest, AuditPathResolver, AuditStore};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let state_root: PathBuf = args
        .next()
        .expect("usage: audit_store_create_probe <state-root> <project-root>")
        .into();
    let project_root: PathBuf = args
        .next()
        .expect("usage: audit_store_create_probe <state-root> <project-root>")
        .into();

    println!("READY");
    std::io::stdout().flush().expect("stdout must flush");

    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .expect("reading the go signal must succeed");

    let storage_path = AuditPathResolver
        .resolve(AuditPathRequest::new(&state_root, vec![project_root]))
        .expect("resolving the audit storage path must succeed");

    match AuditStore::open(storage_path) {
        Ok(_store) => println!("OK"),
        Err(error) => println!("ERR {error:?}"),
    }
    std::io::stdout().flush().expect("stdout must flush");
}
