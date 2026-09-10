//! Cross-`rusqlite`-version compatibility check, kept deliberately (response
//! 374, R2): every `rusqlite` minor carries a new bundled SQLite, and this
//! is the check that catches a storage-engine change an older engine
//! cannot read back -- a property of SQLite itself, not of this crate's
//! code, so it recurs on someone else's release schedule rather than
//! ours. It cannot be a normal `#[test]`: only one `rusqlite` version can
//! be compiled into one `Cargo.lock` at a time, so verifying "a store
//! written by the new engine survives a downgrade to the old one" needs
//! two separate builds with two separate lockfile pins in between.
//!
//! Writes a couple of real audit records, through the crate's own real
//! `AuditStore` API, to a store at the path given on the command line.
//!
//! ## Procedure, next time a `rusqlite`/bundled-SQLite bump lands
//!
//! 1. At the new pin (post-bump), `cargo run --example
//!    audit_store_cross_version_write -- <dir-a>`.
//! 2. Re-pin `Cargo.toml`'s `rusqlite` back to the previous version, run
//!    `cargo update -p rusqlite --precise <old> -p libsqlite3-sys
//!    --precise <old>`, rebuild, then `cargo run --example
//!    audit_store_cross_version_read -- <dir-a>` -- this is the direction
//!    that can lose a user's records after downgrading Tekstide.
//! 3. Still at the old pin, `cargo run --example
//!    audit_store_cross_version_write -- <dir-b>`.
//! 4. Restore `Cargo.toml`/`Cargo.lock` to the new pin, rebuild, then
//!    `cargo run --example audit_store_cross_version_read -- <dir-b>`.
//! 5. Both reads must print `OK`. Restore the new pin as the final state
//!    either way, and record the result in the slice's own evidence --
//!    if it fails, that does not necessarily block the bump, but the
//!    changelog must say so plainly (per this file's own handoff,
//!    `rfcs/handoffs/dependency-currency-0.18.md`, check 2).

use std::path::PathBuf;

use tekstide_core::audit::{
    AuditActionKind, AuditActionSource, AuditActorKind, AuditEventFamily, AuditOutcome,
    AuditPathRequest, AuditPathResolver, AuditStore, DurableAuditRecordV1,
};
use tekstide_core::project::ProjectId;

fn main() {
    let state_root: PathBuf = std::env::args()
        .nth(1)
        .expect("usage: audit_store_cross_version_write <state_root>")
        .into();
    std::fs::create_dir_all(&state_root).expect("state root must be creatable");

    let storage_path = AuditPathResolver
        .resolve(AuditPathRequest::new(state_root, Vec::new()))
        .expect("a fresh, absolute state root must resolve");

    let mut store = AuditStore::open(storage_path).expect("a fresh store must open");

    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::ProjectAdded,
        AuditOutcome::Applied,
        AuditActionKind::ProjectAdd,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    record.project_id = Some(ProjectId::new_uuid());
    store.append(&record).expect("a valid record must append");

    let mut second = DurableAuditRecordV1::new(
        AuditEventFamily::ProjectAdded,
        AuditOutcome::Applied,
        AuditActionKind::ProjectAdd,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    second.project_id = Some(ProjectId::new_uuid());
    store
        .append(&second)
        .expect("a second valid record must append");

    println!(
        "wrote 2 records under bundled SQLite {}",
        rusqlite::version()
    );
}
