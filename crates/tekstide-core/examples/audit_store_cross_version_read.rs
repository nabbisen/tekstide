//! Companion to `audit_store_cross_version_write` (see that file's own
//! header for why this pair is kept and the full procedure). Opens and
//! queries a store at the path given on the command line, under
//! whichever `rusqlite`/bundled-SQLite version this binary was built
//! with -- normally a different version than whatever wrote it. Exits
//! non-zero and prints the error if the store will not open or the
//! records do not read back; prints `OK` on success.

use std::path::PathBuf;

use tekstide_core::audit::{AuditPathRequest, AuditPathResolver, AuditQuery, AuditStore};

fn main() {
    let state_root: PathBuf = std::env::args()
        .nth(1)
        .expect("usage: audit_store_cross_version_read <state_root>")
        .into();

    let storage_path = AuditPathResolver
        .resolve(AuditPathRequest::new(state_root, Vec::new()))
        .expect("the same state root the writer used must still resolve");

    let store = match AuditStore::open(storage_path) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("FAILED TO OPEN: {error:?}");
            std::process::exit(1);
        }
    };

    let page = match store.query(&AuditQuery::latest(10)) {
        Ok(page) => page,
        Err(error) => {
            eprintln!("FAILED TO QUERY: {error:?}");
            std::process::exit(1);
        }
    };

    println!(
        "opened under bundled SQLite {} -- read back {} record(s)",
        rusqlite::version(),
        page.records.len()
    );
    for record in &page.records {
        println!("  {:?} {:?}", record.record.family, record.record.outcome);
    }
    assert_eq!(
        page.records.len(),
        2,
        "expected exactly the 2 records the writer wrote"
    );
    println!("OK");
}
