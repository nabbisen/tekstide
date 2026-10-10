//! RFC-058 D1: "a project that already satisfies a requirement in one
//! place should not grow a second mechanism beside it. Confirm it; do not
//! duplicate it." This confirms it.
//!
//! Two independent [`AuditStore`] handles on the same real sqlite file,
//! synchronized on a barrier so both reach [`AuditStore::append`] at
//! approximately the same instant -- the real contention `busy_timeout`
//! (RFC-013's own `BUSY_TIMEOUT`) exists to serialize, not reasoned about
//! from the pragma alone. Two independent connections rather than two real
//! OS processes: SQLite's own file locking is enforced by the kernel
//! against the open file descriptor, not against a process identity, so a
//! second connection in this same test process exercises the identical
//! mechanism a second real Tekstide process would -- the same reasoning
//! `transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped`
//! already relies on for `flock` (open a second, independent handle, not a
//! second process) to prove a lock is OS-visible rather than in-memory.

use std::sync::Barrier;

use crate::audit::tests::support::{TestAuditDirs, project_added};
use crate::audit::{AuditAppendOutcome, AuditQuery, AuditStore};
use crate::project::ProjectId;

#[test]
fn two_concurrent_writers_against_the_same_store_both_succeed() {
    let dirs = TestAuditDirs::new("two-live-writers");
    // Establish the schema with one handle first, then close it. D1's own
    // claim is about *writers against an established store*, not about two
    // processes racing to create the database file for the first time --
    // SQLite's own schema-creation race on a brand-new file is a narrower,
    // separate hazard this test deliberately does not conflate with it.
    drop(AuditStore::open(dirs.storage_path.clone()).expect("establish the schema once"));

    let barrier = std::sync::Arc::new(Barrier::new(2));

    let storage_path_a = dirs.storage_path.clone();
    let barrier_a = barrier.clone();
    let project_a = ProjectId::new_uuid();
    let writer_a = std::thread::spawn(move || {
        let mut store = AuditStore::open(storage_path_a).expect("open a real audit store");
        barrier_a.wait();
        store.append(&project_added(project_a))
    });

    let storage_path_b = dirs.storage_path.clone();
    let barrier_b = barrier.clone();
    let project_b = ProjectId::new_uuid();
    let writer_b = std::thread::spawn(move || {
        let mut store = AuditStore::open(storage_path_b).expect("open a real audit store");
        barrier_b.wait();
        store.append(&project_added(project_b))
    });

    let result_a = writer_a.join().expect("writer thread A must not panic");
    let result_b = writer_b.join().expect("writer thread B must not panic");

    assert!(
        matches!(result_a, Ok(AuditAppendOutcome::Appended { .. })),
        "the first concurrent writer must succeed, not error as if the store were corrupted \
         or exclusively locked: {result_a:?}"
    );
    assert!(
        matches!(result_b, Ok(AuditAppendOutcome::Appended { .. })),
        "the second concurrent writer must succeed too -- `busy_timeout` serializes contention \
         rather than failing it: {result_b:?}"
    );

    let reader = AuditStore::open(dirs.storage_path.clone()).expect("open a real audit store");
    let page = reader
        .query(&AuditQuery::latest(10))
        .expect("querying the store after two concurrent writers must succeed");
    assert_eq!(
        page.records.len(),
        2,
        "both concurrent writers' records must be present, neither lost nor duplicated: \
         {:?}",
        page.records
    );
}
