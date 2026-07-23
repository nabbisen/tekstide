use std::fs;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};

use rusqlite::Connection;

use crate::audit::recovery::{recover_with_move, recover_with_move_and_initializer};
use crate::audit::{
    AuditArtifactKind, AuditArtifactStatus, AuditDiagnosticStatus, AuditDiagnostics, AuditQuery,
    AuditRecovery, AuditRecoveryErrorReason, AuditStore, AuditStoreErrorReason,
};

use super::support::TestAuditDirs;

#[test]
fn recovery_quarantines_complete_artifact_set_and_records_fresh_event() {
    let dirs = TestAuditDirs::new("recovery-complete");
    create_corrupt_artifacts(&dirs, true);

    let receipt = AuditRecovery.recover(dirs.storage_path.clone()).unwrap();

    assert!(receipt.recovery_event_recorded);
    assert_eq!(receipt.entries.len(), 4);
    assert!(
        receipt
            .entries
            .iter()
            .all(|entry| entry.status == AuditArtifactStatus::Moved)
    );
    let bundle = dirs
        .storage_path
        .recovery_dir()
        .join(receipt.recovery_id.as_str());
    for name in [
        "audit.sqlite3",
        "audit.sqlite3-journal",
        "audit.sqlite3-wal",
        "audit.sqlite3-shm",
        "manifest.json",
    ] {
        assert!(bundle.join(name).is_file(), "missing {name}");
    }
    let manifest = fs::read_to_string(bundle.join("manifest.json")).unwrap();
    assert!(manifest.contains("\"complete\": true"));
    assert!(!manifest.contains("private-artifact-sentinel"));

    let store = AuditStore::open(dirs.storage_path.clone()).unwrap();
    let page = store.query(&AuditQuery::latest(10)).unwrap();
    assert_eq!(page.records.len(), 1);
    assert_eq!(
        page.records[0].record.family,
        crate::audit::AuditEventFamily::AuditStoreRecovery
    );
}

#[test]
fn recovery_manifest_records_absent_companions() {
    let dirs = TestAuditDirs::new("recovery-absent");
    create_corrupt_artifacts(&dirs, false);

    let receipt = AuditRecovery.recover(dirs.storage_path.clone()).unwrap();

    assert_eq!(
        status(&receipt.entries, AuditArtifactKind::Database),
        AuditArtifactStatus::Moved
    );
    for kind in [
        AuditArtifactKind::RollbackJournal,
        AuditArtifactKind::Wal,
        AuditArtifactKind::SharedMemory,
    ] {
        assert_eq!(status(&receipt.entries, kind), AuditArtifactStatus::Absent);
    }
}

#[test]
fn partial_quarantine_writes_failure_manifest_and_does_not_create_fresh_store() {
    let dirs = TestAuditDirs::new("recovery-partial");
    create_corrupt_artifacts(&dirs, true);
    let wal_file = dirs.storage_path.wal_file();

    let error = recover_with_move(dirs.storage_path.clone(), |source, destination| {
        if source == wal_file {
            Err(io::Error::other("injected move failure"))
        } else {
            fs::rename(source, destination)
        }
    })
    .unwrap_err();

    assert_eq!(error.reason, AuditRecoveryErrorReason::QuarantineIncomplete);
    assert_eq!(
        status(&error.entries, AuditArtifactKind::Wal),
        AuditArtifactStatus::Failed
    );
    assert!(wal_file.exists());
    assert!(!dirs.storage_path.database_file().exists());
    let bundle = dirs
        .storage_path
        .recovery_dir()
        .join(error.recovery_id.unwrap().as_str());
    let manifest = fs::read_to_string(bundle.join("manifest.json")).unwrap();
    assert!(manifest.contains("\"complete\": false"));
    assert!(manifest.contains("\"status\": \"failed\""));

    assert_open_blocked(&dirs);
    let receipt = AuditRecovery.resume(dirs.storage_path.clone()).unwrap();
    assert!(receipt.recovery_event_recorded);
    assert!(!dirs.storage_path.recovery_marker_file().exists());
    AuditStore::open(dirs.storage_path.clone()).unwrap();
}

#[test]
fn manifest_write_failure_keeps_restart_guard_and_can_resume() {
    let dirs = TestAuditDirs::new("recovery-manifest-failure");
    create_corrupt_artifacts(&dirs, true);
    let database_file = dirs.storage_path.database_file().to_path_buf();

    let error = recover_with_move(dirs.storage_path.clone(), |source, destination| {
        fs::rename(source, destination)?;
        if source == database_file {
            fs::create_dir(destination.parent().unwrap().join(".manifest.json.tmp"))?;
        }
        Ok(())
    })
    .unwrap_err();

    assert_eq!(error.reason, AuditRecoveryErrorReason::ManifestWrite);
    assert_open_blocked(&dirs);
    let bundle = dirs
        .storage_path
        .recovery_dir()
        .join(error.recovery_id.unwrap().as_str());
    fs::remove_dir(bundle.join(".manifest.json.tmp")).unwrap();

    let receipt = AuditRecovery.resume(dirs.storage_path.clone()).unwrap();
    assert!(receipt.recovery_event_recorded);
    AuditStore::open(dirs.storage_path.clone()).unwrap();
}

#[test]
fn interruption_during_artifact_moves_keeps_restart_guard_and_can_resume() {
    let dirs = TestAuditDirs::new("recovery-interrupted-move");
    create_corrupt_artifacts(&dirs, true);
    let database_file = dirs.storage_path.database_file().to_path_buf();

    let interrupted = catch_unwind(AssertUnwindSafe(|| {
        let _ = recover_with_move(dirs.storage_path.clone(), |source, destination| {
            fs::rename(source, destination)?;
            if source == database_file {
                panic!("simulated process interruption");
            }
            Ok(())
        });
    }));

    assert!(interrupted.is_err());
    assert_open_blocked(&dirs);
    let receipt = AuditRecovery.resume(dirs.storage_path.clone()).unwrap();
    assert!(receipt.recovery_event_recorded);
    AuditStore::open(dirs.storage_path.clone()).unwrap();
}

#[test]
fn failed_fresh_initialization_keeps_restart_guard_and_can_resume() {
    let dirs = TestAuditDirs::new("recovery-initialization-failure");
    create_corrupt_artifacts(&dirs, false);
    let initialization_path = dirs.storage_path.recovery_initialization_path();

    let error = recover_with_move_and_initializer(
        dirs.storage_path.clone(),
        |source, destination| fs::rename(source, destination),
        |path, _| {
            for artifact in [
                path.database_file().to_path_buf(),
                path.journal_file(),
                path.wal_file(),
                path.shared_memory_file(),
            ] {
                fs::write(artifact, b"incomplete fresh attempt")?;
            }
            Err(io::Error::other("injected initialization failure"))
        },
    )
    .unwrap_err();

    assert_eq!(error.reason, AuditRecoveryErrorReason::FreshStore);
    assert_open_blocked(&dirs);
    assert!(initialization_path.database_file().is_file());
    assert!(initialization_path.journal_file().is_file());
    assert!(initialization_path.wal_file().is_file());
    assert!(initialization_path.shared_memory_file().is_file());

    let receipt = AuditRecovery.resume(dirs.storage_path.clone()).unwrap();
    assert!(receipt.recovery_event_recorded);
    assert!(!initialization_path.database_file().exists());
    assert!(!initialization_path.journal_file().exists());
    assert!(!initialization_path.wal_file().exists());
    assert!(!initialization_path.shared_memory_file().exists());
    AuditStore::open(dirs.storage_path.clone()).unwrap();
}

#[test]
fn resume_recovers_partial_canonical_initialization_artifacts() {
    let dirs = TestAuditDirs::new("recovery-canonical-initialization-failure");
    create_corrupt_artifacts(&dirs, false);
    let canonical_path = dirs.storage_path.clone();

    let error = recover_with_move_and_initializer(
        dirs.storage_path.clone(),
        |source, destination| fs::rename(source, destination),
        move |_, _| {
            for artifact in [
                canonical_path.database_file().to_path_buf(),
                canonical_path.journal_file(),
                canonical_path.wal_file(),
                canonical_path.shared_memory_file(),
            ] {
                fs::write(artifact, [])?;
            }
            Err(io::Error::other(
                "injected canonical initialization failure",
            ))
        },
    )
    .unwrap_err();

    assert_eq!(error.reason, AuditRecoveryErrorReason::FreshStore);
    let open_error = match AuditStore::open(dirs.storage_path.clone()) {
        Ok(_) => panic!("ordinary open must not bypass active recovery"),
        Err(error) => error,
    };
    assert_eq!(open_error.reason, AuditStoreErrorReason::RecoveryIncomplete);
    assert!(dirs.storage_path.database_file().is_file());
    assert!(dirs.storage_path.journal_file().is_file());
    assert!(dirs.storage_path.wal_file().is_file());
    assert!(dirs.storage_path.shared_memory_file().is_file());

    let receipt = AuditRecovery.resume(dirs.storage_path.clone()).unwrap();
    assert!(receipt.recovery_event_recorded);
    AuditStore::open(dirs.storage_path.clone()).unwrap();
}

#[test]
fn recovery_refuses_healthy_foreign_future_and_missing_stores() {
    let healthy = TestAuditDirs::new("recovery-refuse-healthy");
    {
        let store = AuditStore::open(healthy.storage_path.clone()).unwrap();
        drop(store);
    }
    assert_not_recoverable(&healthy);

    let missing = TestAuditDirs::new("recovery-refuse-missing");
    assert_not_recoverable(&missing);

    for (label, pragma, expected) in [
        (
            "recovery-refuse-foreign",
            "PRAGMA application_id = 12345",
            AuditDiagnosticStatus::UnsupportedApplication,
        ),
        (
            "recovery-refuse-future",
            "PRAGMA user_version = 2",
            AuditDiagnosticStatus::UnsupportedSchema,
        ),
    ] {
        let dirs = TestAuditDirs::new(label);
        {
            let store = AuditStore::open(dirs.storage_path.clone()).unwrap();
            drop(store);
        }
        let connection = Connection::open(dirs.storage_path.database_file()).unwrap();
        connection.execute_batch(pragma).unwrap();
        drop(connection);
        let before = fs::read(dirs.storage_path.database_file()).unwrap();
        assert_eq!(AuditDiagnostics.run(&dirs.storage_path).status, expected);
        assert_not_recoverable(&dirs);
        assert_eq!(fs::read(dirs.storage_path.database_file()).unwrap(), before);
    }
}

fn create_corrupt_artifacts(dirs: &TestAuditDirs, companions: bool) {
    fs::create_dir_all(dirs.storage_path.audit_dir()).unwrap();
    fs::write(
        dirs.storage_path.database_file(),
        b"not sqlite private-artifact-sentinel",
    )
    .unwrap();
    if companions {
        for path in [
            dirs.storage_path.journal_file(),
            dirs.storage_path.wal_file(),
            dirs.storage_path.shared_memory_file(),
        ] {
            fs::write(path, []).unwrap();
        }
    }
}

fn assert_not_recoverable(dirs: &TestAuditDirs) {
    let error = AuditRecovery
        .recover(dirs.storage_path.clone())
        .unwrap_err();
    assert_eq!(error.reason, AuditRecoveryErrorReason::StoreNotRecoverable);
}

fn assert_open_blocked(dirs: &TestAuditDirs) {
    let error = match AuditStore::open(dirs.storage_path.clone()) {
        Ok(_) => panic!("ordinary open must not bypass active recovery"),
        Err(error) => error,
    };
    assert_eq!(error.reason, AuditStoreErrorReason::RecoveryIncomplete);
    assert!(!dirs.storage_path.database_file().exists());
}

fn status(
    entries: &[crate::audit::AuditRecoveryEntry],
    kind: AuditArtifactKind,
) -> AuditArtifactStatus {
    entries
        .iter()
        .find(|entry| entry.kind == kind)
        .unwrap()
        .status
}
