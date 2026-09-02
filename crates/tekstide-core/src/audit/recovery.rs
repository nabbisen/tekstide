use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::diagnostics::{AuditDiagnosticStatus, AuditDiagnostics};
use super::path::AuditStoragePath;
use super::record::{
    AuditActionKind, AuditActionSource, AuditActorKind, AuditEventFamily, AuditOutcome,
    AuditReasonCode, AuditReference, AuditSubjectKind, DurableAuditRecordV1,
};
use super::store::{AuditQuery, AuditStore, AuditStoreErrorReason};
use crate::domain::AuditEventId;

const RECOVERY_STATE_VERSION: u32 = 1;
const RECOVERY_MANIFEST_VERSION: u32 = 1;
const MARKER_TEMP_FILE_NAME: &str = ".active-recovery.json.tmp";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const MANIFEST_TEMP_FILE_NAME: &str = ".manifest.json.tmp";
const MAX_STATE_FILE_BYTES: u64 = 16 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditArtifactKind {
    Database,
    RollbackJournal,
    Wal,
    SharedMemory,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditArtifactStatus {
    Moved,
    Absent,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AuditRecoveryEntry {
    pub kind: AuditArtifactKind,
    pub status: AuditArtifactStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecoveryReceipt {
    pub recovery_id: AuditReference,
    pub entries: Vec<AuditRecoveryEntry>,
    pub recovery_event_recorded: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditRecoveryErrorReason {
    Path,
    StoreNotRecoverable,
    RecoveryInProgress,
    RecoveryDirectory,
    RecoveryState,
    QuarantineIncomplete,
    ManifestWrite,
    FreshStore,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecoveryError {
    pub reason: AuditRecoveryErrorReason,
    pub recovery_id: Option<AuditReference>,
    pub entries: Vec<AuditRecoveryEntry>,
}

impl AuditRecoveryError {
    fn new(reason: AuditRecoveryErrorReason) -> Self {
        Self {
            reason,
            recovery_id: None,
            entries: Vec::new(),
        }
    }

    fn with_recovery(
        reason: AuditRecoveryErrorReason,
        recovery_id: AuditReference,
        entries: Vec<AuditRecoveryEntry>,
    ) -> Self {
        Self {
            reason,
            recovery_id: Some(recovery_id),
            entries,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AuditRecovery;

impl AuditRecovery {
    /// The caller must close all application-owned handles before recovery starts.
    pub fn recover(
        self,
        storage_path: AuditStoragePath,
    ) -> Result<AuditRecoveryReceipt, AuditRecoveryError> {
        recover_with_move(storage_path, |source, destination| {
            fs::rename(source, destination)
        })
    }

    /// Retries the exact recovery identified by the durable active-recovery marker.
    pub fn resume(
        self,
        storage_path: AuditStoragePath,
    ) -> Result<AuditRecoveryReceipt, AuditRecoveryError> {
        resume_with_move(storage_path, |source, destination| {
            fs::rename(source, destination)
        })
    }

    /// RFC-047 PR-047-B, D2: [`Self::recover`] plus the two things a
    /// caller actually needs afterward -- a real, opened `AuditStore`
    /// (recovery itself leaves the store closed; `finish_recovery`'s own
    /// internal open, inside [`initialize_fresh_database`], is for the
    /// atomic-install step only and does not outlive it) and the
    /// directory the unreadable original was quarantined into, since
    /// D2's whole justification for auto-recovering without asking is
    /// that the old data is moved aside, not destroyed -- a caller that
    /// cannot report *where* has nothing to justify the decision with.
    ///
    /// The quarantine directory is reconstructed from public data only
    /// (`storage_path.recovery_dir()`, `receipt.recovery_id.as_str()`),
    /// the same path [`quarantine_artifacts`] itself writes to -- no
    /// schema change, nothing added to [`AuditRecoveryReceipt`] itself.
    ///
    /// `original_reason` is what `AuditStore::open` reported before this
    /// was called -- used only if recovery itself fails, since
    /// `AuditRecoveryError` has no `AuditStoreErrorReason` of its own and
    /// the original problem is still the truest thing to report.
    pub fn recover_and_reopen(
        self,
        storage_path: AuditStoragePath,
        original_reason: AuditStoreErrorReason,
    ) -> AuditRecoveryOutcome {
        let reopen_path = storage_path.clone();
        match self.recover(storage_path) {
            Ok(receipt) => reopen_after_recovery(
                reopen_path,
                receipt,
                |quarantine_dir, store, recovery_event_recorded| AuditRecoveryOutcome::Recovered {
                    store,
                    quarantine_dir,
                    recovery_event_recorded,
                },
            ),
            Err(_recovery_error) => AuditRecoveryOutcome::Failed(original_reason),
        }
    }

    /// The `resume()` half of [`Self::recover_and_reopen`]. No
    /// `original_reason` parameter -- the one failure this is ever
    /// called for is `AuditStoreErrorReason::RecoveryIncomplete`, so a
    /// failed resume reports that back unambiguously rather than asking
    /// the caller to repeat it.
    pub fn resume_and_reopen(self, storage_path: AuditStoragePath) -> AuditRecoveryOutcome {
        let reopen_path = storage_path.clone();
        match self.resume(storage_path) {
            Ok(receipt) => reopen_after_recovery(
                reopen_path,
                receipt,
                |_quarantine_dir, store, recovery_event_recorded| AuditRecoveryOutcome::Resumed {
                    store,
                    recovery_event_recorded,
                },
            ),
            Err(_recovery_error) => {
                AuditRecoveryOutcome::Failed(AuditStoreErrorReason::RecoveryIncomplete)
            }
        }
    }
}

/// RFC-047 PR-047-B: what a caller needs to know after asking
/// [`AuditRecovery`] to fix a store that would not open on its own.
/// Not `Debug` -- `AuditStore` itself is not (it holds a live SQLite
/// connection), so a caller that wants to inspect this in a test
/// matches its fields directly rather than formatting the whole value.
pub enum AuditRecoveryOutcome {
    Resumed {
        store: AuditStore,
        /// Whether `AuditStoreRecovery`'s own durable record made it into
        /// the store. `false` means the store works but the *disclosure*
        /// of what happened did not survive -- still a real, reportable
        /// health problem (§4 of the risk document: "do not claim more
        /// than the record supports").
        recovery_event_recorded: bool,
    },
    Recovered {
        store: AuditStore,
        /// Where the unreadable original went -- D2's own condition for
        /// auto-recovering without asking first.
        quarantine_dir: PathBuf,
        recovery_event_recorded: bool,
    },
    /// Recovery was attempted and did not produce a usable store --
    /// either `recover()`/`resume()` itself failed, or it succeeded but
    /// the store still would not reopen afterward.
    Failed(AuditStoreErrorReason),
}

/// Shared by both [`AuditRecovery::recover_and_reopen`] and
/// [`AuditRecovery::resume_and_reopen`]: reopen the now-recovered store
/// with the ordinary, public [`AuditStore::open`] -- safe here
/// specifically because [`finish_recovery`] already removed the active
/// recovery marker before returning, so `recovery_is_active()` reads
/// `false` by the time this runs, unlike the internal atomic-install
/// step's own use of `open_after_complete_recovery`.
fn reopen_after_recovery(
    storage_path: AuditStoragePath,
    receipt: AuditRecoveryReceipt,
    into_outcome: impl FnOnce(PathBuf, AuditStore, bool) -> AuditRecoveryOutcome,
) -> AuditRecoveryOutcome {
    let quarantine_dir = storage_path
        .recovery_dir()
        .join(receipt.recovery_id.as_str());
    match AuditStore::open(storage_path) {
        Ok(store) => into_outcome(quarantine_dir, store, receipt.recovery_event_recorded),
        Err(reopen_error) => AuditRecoveryOutcome::Failed(reopen_error.reason),
    }
}

pub(crate) fn recover_with_move(
    storage_path: AuditStoragePath,
    move_artifact: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<AuditRecoveryReceipt, AuditRecoveryError> {
    recover_with_move_and_initializer(storage_path, move_artifact, initialize_fresh_database)
}

/// RFC-047 PR-047-B: the one way another crate's own tests can reach a
/// genuinely resumable (not merely a bare, unstartable marker file) --
/// mirrors this module's own
/// `manifest_write_failure_keeps_restart_guard_and_can_resume` test
/// exactly (inject a failure after the database artifact has already
/// moved, leaving the active-recovery marker in place), since that
/// private technique is the only way to reach this state at all; a
/// hand-rolled marker/bundle pair would not match the internal format
/// `resume()` itself expects and would fail for the wrong reason.
///
/// `#[cfg(any(test, feature = "test-support"))]`, the same gate
/// `runtime::terminal::launch`'s own leak guard and RFC-036's
/// `ProjectSession::add_transcript` already use to cross this exact
/// crate boundary -- `tekstide`'s own `Cargo.toml` already enables
/// `test-support` for its `[dev-dependencies]`.
#[cfg(any(test, feature = "test-support"))]
pub fn corrupt_and_interrupt_recovery_for_test(storage_path: &AuditStoragePath) {
    use std::fs;

    fs::create_dir_all(storage_path.audit_dir()).expect("create audit dir for test corruption");
    fs::write(
        storage_path.database_file(),
        b"not sqlite -- test-support corruption fixture",
    )
    .expect("write corrupt database for test");
    let database_file = storage_path.database_file().to_path_buf();
    let error = recover_with_move(storage_path.clone(), move |source, destination| {
        fs::rename(source, destination)?;
        if source == database_file {
            fs::create_dir(destination.parent().unwrap().join(".manifest.json.tmp"))?;
        }
        Ok(())
    })
    .expect_err("the injected manifest-write failure must interrupt recovery");
    let bundle = storage_path
        .recovery_dir()
        .join(error.recovery_id.expect("a bundle must exist").as_str());
    fs::remove_dir(bundle.join(".manifest.json.tmp"))
        .expect("clear the injected obstruction so resume() can complete");
}

/// RFC-047 PR-047-B response 358, R2: the one way another crate's own
/// tests can reach `recovery_event_recorded: false` on a *successful*
/// recovery. Organically failing [`initialize_fresh_database`]'s own
/// `store.append(...)` on a database this same call just created and
/// validated has no reliable, portable filesystem trick behind it --
/// WAL-mode SQLite gives a black-box test nothing to pull on that would
/// not also risk `prepare_for_atomic_install` or the diagnostics check
/// right after it, taking down the whole recovery instead of leaving it
/// successful-but-unrecorded.
///
/// So this runs the real quarantine/move/reopen path
/// (`recover_with_move_and_initializer`, the same seam
/// [`initialize_fresh_database`] is itself plugged into -- not a
/// separate mock) with a substitute initializer that does everything
/// [`initialize_fresh_database`] does -- real store, real schema, real
/// `prepare_for_atomic_install` -- except it discards the append's own
/// result and always reports `Ok(false)`. The record write is still
/// attempted for real; only the boolean a caller reads back is forced,
/// simulating the one input this module's own callers cannot be handed
/// any other way. Documented as simulated rather than passed off as
/// organic, the same honesty
/// [`corrupt_and_interrupt_recovery_for_test`] already holds itself to.
///
/// `#[cfg(any(test, feature = "test-support"))]`, same gate as
/// [`corrupt_and_interrupt_recovery_for_test`] -- `tekstide`'s own
/// `Cargo.toml` already enables `test-support` for its
/// `[dev-dependencies]`.
#[cfg(any(test, feature = "test-support"))]
pub fn recover_and_reopen_forcing_unrecorded_event_for_test(
    storage_path: AuditStoragePath,
) -> AuditRecoveryOutcome {
    let reopen_path = storage_path.clone();
    let outcome = recover_with_move_and_initializer(
        storage_path,
        |source, destination| fs::rename(source, destination),
        |initialization_path, recovery_id| {
            let mut store =
                AuditStore::open_after_complete_recovery(initialization_path.clone())
                    .map_err(|_| io::Error::other("fresh audit store initialization failed"))?;
            let _ = store.append(&recovery_record(recovery_id));
            store
                .prepare_for_atomic_install()
                .map_err(|_| io::Error::other("fresh audit store finalization failed"))?;
            drop(store);
            Ok(false)
        },
    );
    match outcome {
        Ok(receipt) => reopen_after_recovery(
            reopen_path,
            receipt,
            |quarantine_dir, store, recovery_event_recorded| AuditRecoveryOutcome::Recovered {
                store,
                quarantine_dir,
                recovery_event_recorded,
            },
        ),
        Err(_recovery_error) => AuditRecoveryOutcome::Failed(AuditStoreErrorReason::Corrupt),
    }
}

pub(crate) fn recover_with_move_and_initializer(
    storage_path: AuditStoragePath,
    move_artifact: impl FnMut(&Path, &Path) -> io::Result<()>,
    initialize_fresh: impl FnOnce(&AuditStoragePath, &AuditReference) -> io::Result<bool>,
) -> Result<AuditRecoveryReceipt, AuditRecoveryError> {
    validate_recovery_path(&storage_path)?;
    if storage_path
        .recovery_is_active()
        .map_err(|_| AuditRecoveryError::new(AuditRecoveryErrorReason::Path))?
    {
        return Err(AuditRecoveryError::new(
            AuditRecoveryErrorReason::RecoveryInProgress,
        ));
    }
    if !matches!(
        AuditDiagnostics.run(&storage_path).status,
        AuditDiagnosticStatus::Corrupt | AuditDiagnosticStatus::InvalidRecords
    ) {
        return Err(AuditRecoveryError::new(
            AuditRecoveryErrorReason::StoreNotRecoverable,
        ));
    }

    fs::create_dir_all(storage_path.recovery_dir())
        .map_err(|_| AuditRecoveryError::new(AuditRecoveryErrorReason::RecoveryDirectory))?;
    validate_recovery_path(&storage_path)?;
    let (recovery_id, bundle_dir) = create_unique_bundle(&storage_path)?;
    write_active_marker(&storage_path, &recovery_id).map_err(|_| {
        AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::RecoveryState,
            recovery_id.clone(),
            Vec::new(),
        )
    })?;

    continue_quarantine(
        storage_path,
        recovery_id,
        bundle_dir,
        move_artifact,
        initialize_fresh,
    )
}

fn resume_with_move(
    storage_path: AuditStoragePath,
    move_artifact: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<AuditRecoveryReceipt, AuditRecoveryError> {
    validate_recovery_path(&storage_path)?;
    let recovery_id = read_active_marker(&storage_path)?;
    let bundle_dir = validate_bundle_dir(&storage_path, &recovery_id)?;

    match AuditDiagnostics.run(&storage_path).status {
        AuditDiagnosticStatus::Missing
        | AuditDiagnosticStatus::Corrupt
        | AuditDiagnosticStatus::InvalidRecords => continue_quarantine(
            storage_path,
            recovery_id,
            bundle_dir,
            move_artifact,
            initialize_fresh_database,
        ),
        AuditDiagnosticStatus::Healthy => {
            let entries = read_complete_manifest(&bundle_dir, &recovery_id)?;
            finish_recovery(
                storage_path,
                recovery_id,
                entries,
                initialize_fresh_database,
            )
        }
        _ => {
            let entries = read_complete_manifest(&bundle_dir, &recovery_id)?;
            clear_artifacts(&artifact_paths(&storage_path)).map_err(|_| {
                AuditRecoveryError::with_recovery(
                    AuditRecoveryErrorReason::FreshStore,
                    recovery_id.clone(),
                    entries.clone(),
                )
            })?;
            finish_recovery(
                storage_path,
                recovery_id,
                entries,
                initialize_fresh_database,
            )
        }
    }
}

fn continue_quarantine(
    storage_path: AuditStoragePath,
    recovery_id: AuditReference,
    bundle_dir: PathBuf,
    move_artifact: impl FnMut(&Path, &Path) -> io::Result<()>,
    initialize_fresh: impl FnOnce(&AuditStoragePath, &AuditReference) -> io::Result<bool>,
) -> Result<AuditRecoveryReceipt, AuditRecoveryError> {
    let artifacts = artifact_paths(&storage_path);
    let entries = quarantine_artifacts(&artifacts, &bundle_dir, move_artifact);
    let complete = entries
        .iter()
        .all(|entry| entry.status != AuditArtifactStatus::Failed);
    write_manifest(&bundle_dir, &recovery_id, complete, &entries).map_err(|_| {
        AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::ManifestWrite,
            recovery_id.clone(),
            entries.clone(),
        )
    })?;
    if !complete {
        return Err(AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::QuarantineIncomplete,
            recovery_id,
            entries,
        ));
    }

    finish_recovery(storage_path, recovery_id, entries, initialize_fresh)
}

fn finish_recovery(
    storage_path: AuditStoragePath,
    recovery_id: AuditReference,
    entries: Vec<AuditRecoveryEntry>,
    initialize_fresh: impl FnOnce(&AuditStoragePath, &AuditReference) -> io::Result<bool>,
) -> Result<AuditRecoveryReceipt, AuditRecoveryError> {
    // An installed database means resume was interrupted after the atomic rename.
    let event_recorded = if storage_path.database_file().exists() {
        let store =
            AuditStore::open_after_complete_recovery(storage_path.clone()).map_err(|_| {
                AuditRecoveryError::with_recovery(
                    AuditRecoveryErrorReason::FreshStore,
                    recovery_id.clone(),
                    entries.clone(),
                )
            })?;
        recovery_event_exists(&store, &recovery_id)
    } else {
        let initialization_path = storage_path.recovery_initialization_path();
        clear_artifacts(&artifact_paths(&initialization_path)).map_err(|_| {
            AuditRecoveryError::with_recovery(
                AuditRecoveryErrorReason::FreshStore,
                recovery_id.clone(),
                entries.clone(),
            )
        })?;
        let event_recorded =
            initialize_fresh(&initialization_path, &recovery_id).map_err(|_| {
                AuditRecoveryError::with_recovery(
                    AuditRecoveryErrorReason::FreshStore,
                    recovery_id.clone(),
                    entries.clone(),
                )
            })?;
        if AuditDiagnostics.run(&initialization_path).status != AuditDiagnosticStatus::Healthy
            || initialization_path.journal_file().exists()
            || initialization_path.wal_file().exists()
            || initialization_path.shared_memory_file().exists()
        {
            return Err(AuditRecoveryError::with_recovery(
                AuditRecoveryErrorReason::FreshStore,
                recovery_id,
                entries,
            ));
        }
        fs::rename(
            initialization_path.database_file(),
            storage_path.database_file(),
        )
        .and_then(|()| sync_directory(storage_path.audit_dir()))
        .map_err(|_| {
            AuditRecoveryError::with_recovery(
                AuditRecoveryErrorReason::FreshStore,
                recovery_id.clone(),
                entries.clone(),
            )
        })?;
        event_recorded
    };
    fs::remove_file(storage_path.recovery_marker_file()).map_err(|_| {
        AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::RecoveryState,
            recovery_id.clone(),
            entries.clone(),
        )
    })?;
    let _ = sync_directory(storage_path.recovery_dir());
    Ok(AuditRecoveryReceipt {
        recovery_id,
        entries,
        recovery_event_recorded: event_recorded,
    })
}

fn initialize_fresh_database(
    storage_path: &AuditStoragePath,
    recovery_id: &AuditReference,
) -> io::Result<bool> {
    let mut store = AuditStore::open_after_complete_recovery(storage_path.clone())
        .map_err(|_| io::Error::other("fresh audit store initialization failed"))?;
    let event_recorded = store.append(&recovery_record(recovery_id)).is_ok();
    store
        .prepare_for_atomic_install()
        .map_err(|_| io::Error::other("fresh audit store finalization failed"))?;
    drop(store);
    Ok(event_recorded)
}

fn clear_artifacts(artifacts: &[(AuditArtifactKind, PathBuf)]) -> io::Result<()> {
    for (_, path) in artifacts {
        remove_optional_regular_file(path)?;
    }
    Ok(())
}

fn validate_recovery_path(storage_path: &AuditStoragePath) -> Result<(), AuditRecoveryError> {
    storage_path
        .validate_for_recovery()
        .map_err(|_| AuditRecoveryError::new(AuditRecoveryErrorReason::Path))
}

fn create_unique_bundle(
    storage_path: &AuditStoragePath,
) -> Result<(AuditReference, PathBuf), AuditRecoveryError> {
    for _ in 0..16 {
        let event_id = AuditEventId::new_uuid();
        let recovery_id = AuditReference::new(event_id.as_str()).expect("audit ids are references");
        let bundle_dir = storage_path.recovery_dir().join(recovery_id.as_str());
        match fs::create_dir(&bundle_dir) {
            Ok(()) => return Ok((recovery_id, bundle_dir)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => {
                return Err(AuditRecoveryError::new(
                    AuditRecoveryErrorReason::RecoveryDirectory,
                ));
            }
        }
    }
    Err(AuditRecoveryError::new(
        AuditRecoveryErrorReason::RecoveryDirectory,
    ))
}

fn validate_bundle_dir(
    storage_path: &AuditStoragePath,
    recovery_id: &AuditReference,
) -> Result<PathBuf, AuditRecoveryError> {
    let bundle_dir = storage_path.recovery_dir().join(recovery_id.as_str());
    let metadata = fs::symlink_metadata(&bundle_dir).map_err(|_| {
        AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::RecoveryState,
            recovery_id.clone(),
            Vec::new(),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::RecoveryState,
            recovery_id.clone(),
            Vec::new(),
        ));
    }
    let canonical = fs::canonicalize(&bundle_dir).map_err(|_| {
        AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::RecoveryState,
            recovery_id.clone(),
            Vec::new(),
        )
    })?;
    let recovery_root = fs::canonicalize(storage_path.recovery_dir()).map_err(|_| {
        AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::RecoveryState,
            recovery_id.clone(),
            Vec::new(),
        )
    })?;
    if !canonical.starts_with(recovery_root) {
        return Err(AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::RecoveryState,
            recovery_id.clone(),
            Vec::new(),
        ));
    }
    Ok(canonical)
}

fn artifact_paths(storage_path: &AuditStoragePath) -> [(AuditArtifactKind, PathBuf); 4] {
    [
        (
            AuditArtifactKind::Database,
            storage_path.database_file().to_path_buf(),
        ),
        (
            AuditArtifactKind::RollbackJournal,
            storage_path.journal_file(),
        ),
        (AuditArtifactKind::Wal, storage_path.wal_file()),
        (
            AuditArtifactKind::SharedMemory,
            storage_path.shared_memory_file(),
        ),
    ]
}

fn quarantine_artifacts(
    artifacts: &[(AuditArtifactKind, PathBuf)],
    bundle_dir: &Path,
    mut move_artifact: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Vec<AuditRecoveryEntry> {
    artifacts
        .iter()
        .map(|(kind, source)| {
            let destination = bundle_dir.join(artifact_file_name(*kind));
            let status = match (regular_file_state(source), regular_file_state(&destination)) {
                (FileState::Absent, FileState::Present) => AuditArtifactStatus::Moved,
                (FileState::Absent, FileState::Absent) => AuditArtifactStatus::Absent,
                (FileState::Present, FileState::Absent) => {
                    match move_artifact(source, &destination) {
                        Ok(()) => AuditArtifactStatus::Moved,
                        Err(_) => AuditArtifactStatus::Failed,
                    }
                }
                _ => AuditArtifactStatus::Failed,
            };
            AuditRecoveryEntry {
                kind: *kind,
                status,
            }
        })
        .collect()
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum FileState {
    Absent,
    Present,
    Unsafe,
}

fn regular_file_state(path: &Path) -> FileState {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => FileState::Absent,
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => {
            FileState::Present
        }
        _ => FileState::Unsafe,
    }
}

fn artifact_file_name(kind: AuditArtifactKind) -> &'static str {
    match kind {
        AuditArtifactKind::Database => "audit.sqlite3",
        AuditArtifactKind::RollbackJournal => "audit.sqlite3-journal",
        AuditArtifactKind::Wal => "audit.sqlite3-wal",
        AuditArtifactKind::SharedMemory => "audit.sqlite3-shm",
    }
}

#[derive(Deserialize, Serialize)]
struct ActiveRecoveryMarker {
    version: u32,
    recovery_id: String,
}

fn write_active_marker(
    storage_path: &AuditStoragePath,
    recovery_id: &AuditReference,
) -> io::Result<()> {
    let marker = ActiveRecoveryMarker {
        version: RECOVERY_STATE_VERSION,
        recovery_id: recovery_id.as_str().to_owned(),
    };
    let temp_file = storage_path.recovery_dir().join(MARKER_TEMP_FILE_NAME);
    remove_optional_regular_file(&temp_file)?;
    write_new_file(
        &temp_file,
        &serde_json::to_vec_pretty(&marker).map_err(io::Error::other)?,
    )?;
    fs::rename(temp_file, storage_path.recovery_marker_file())?;
    sync_directory(storage_path.recovery_dir())
}

fn read_active_marker(
    storage_path: &AuditStoragePath,
) -> Result<AuditReference, AuditRecoveryError> {
    let marker_file = storage_path.recovery_marker_file();
    let bytes = read_bounded_file(&marker_file)
        .map_err(|_| AuditRecoveryError::new(AuditRecoveryErrorReason::RecoveryState))?;
    let marker: ActiveRecoveryMarker = serde_json::from_slice(&bytes)
        .map_err(|_| AuditRecoveryError::new(AuditRecoveryErrorReason::RecoveryState))?;
    if marker.version != RECOVERY_STATE_VERSION {
        return Err(AuditRecoveryError::new(
            AuditRecoveryErrorReason::RecoveryState,
        ));
    }
    AuditReference::new(marker.recovery_id)
        .ok_or_else(|| AuditRecoveryError::new(AuditRecoveryErrorReason::RecoveryState))
}

#[derive(Deserialize, Serialize)]
struct RecoveryManifest {
    version: u32,
    recovery_id: String,
    complete: bool,
    artifacts: Vec<AuditRecoveryEntry>,
}

fn write_manifest(
    bundle_dir: &Path,
    recovery_id: &AuditReference,
    complete: bool,
    entries: &[AuditRecoveryEntry],
) -> io::Result<()> {
    let manifest = RecoveryManifest {
        version: RECOVERY_MANIFEST_VERSION,
        recovery_id: recovery_id.as_str().to_owned(),
        complete,
        artifacts: entries.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?;
    let temp_file = bundle_dir.join(MANIFEST_TEMP_FILE_NAME);
    let manifest_file = bundle_dir.join(MANIFEST_FILE_NAME);
    remove_optional_regular_file(&temp_file)?;
    write_new_file(&temp_file, &bytes)?;
    remove_optional_regular_file(&manifest_file)?;
    fs::rename(temp_file, manifest_file)?;
    sync_directory(bundle_dir)
}

fn read_complete_manifest(
    bundle_dir: &Path,
    recovery_id: &AuditReference,
) -> Result<Vec<AuditRecoveryEntry>, AuditRecoveryError> {
    let bytes = read_bounded_file(&bundle_dir.join(MANIFEST_FILE_NAME)).map_err(|_| {
        AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::RecoveryState,
            recovery_id.clone(),
            Vec::new(),
        )
    })?;
    let manifest: RecoveryManifest = serde_json::from_slice(&bytes).map_err(|_| {
        AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::RecoveryState,
            recovery_id.clone(),
            Vec::new(),
        )
    })?;
    let expected_kinds = [
        AuditArtifactKind::Database,
        AuditArtifactKind::RollbackJournal,
        AuditArtifactKind::Wal,
        AuditArtifactKind::SharedMemory,
    ];
    if manifest.version != RECOVERY_MANIFEST_VERSION
        || manifest.recovery_id != recovery_id.as_str()
        || !manifest.complete
        || manifest.artifacts.len() != expected_kinds.len()
        || manifest
            .artifacts
            .iter()
            .zip(expected_kinds)
            .any(|(entry, kind)| entry.kind != kind || entry.status == AuditArtifactStatus::Failed)
    {
        return Err(AuditRecoveryError::with_recovery(
            AuditRecoveryErrorReason::RecoveryState,
            recovery_id.clone(),
            Vec::new(),
        ));
    }
    Ok(manifest.artifacts)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn remove_optional_regular_file(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => {
            fs::remove_file(path)
        }
        Err(error) => Err(error),
        Ok(_) => Err(io::Error::other(
            "recovery state path is not a regular file",
        )),
    }
}

fn read_bounded_file(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_STATE_FILE_BYTES
    {
        return Err(io::Error::other("invalid recovery state file"));
    }
    fs::read(path)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// audit-store-test-isolation handoff, item 4: judged against the same
/// "size-based query standing in for an identity-based one" shape flagged in
/// the 22 test call sites, and fixed the same way -- `family` moved
/// server-side (`AuditQuery` has no field for recovery ever being
/// project-scoped, since a store recovery is store-wide, not per-project).
/// This narrows the window to *recoveries*, so an unrelated event of any
/// other family newly written after this one can no longer crowd it out of
/// the top 10 -- the same crowding risk the test sites had, just far less
/// likely to bite here (recovery runs once, at startup, in one process, per
/// the handoff's own note).
fn recovery_event_exists(store: &AuditStore, recovery_id: &AuditReference) -> bool {
    store
        .query(&AuditQuery {
            family: Some(AuditEventFamily::AuditStoreRecovery),
            ..AuditQuery::latest(10)
        })
        .is_ok_and(|page| {
            page.records
                .iter()
                .any(|record| record.record.subject_ref.as_ref() == Some(recovery_id))
        })
}

fn recovery_record(recovery_id: &AuditReference) -> DurableAuditRecordV1 {
    let mut record = DurableAuditRecordV1::new(
        AuditEventFamily::AuditStoreRecovery,
        AuditOutcome::Completed,
        AuditActionKind::AuditStoreRecovery,
        AuditActorKind::User,
        AuditActionSource::AppCommand,
    );
    record.subject_kind = Some(AuditSubjectKind::RecoveryBundle);
    record.subject_ref = Some(recovery_id.clone());
    record.reason_code = Some(AuditReasonCode::RecoveryCompleted);
    record
}
