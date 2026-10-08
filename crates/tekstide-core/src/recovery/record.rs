//! RFC-027 PR-027-B: the record itself. One file per dirty document, in its own
//! project-scoped directory (D13, `what-recovery-must-not-do.md` §2 row 6: `0600` in a
//! `0700` directory). **Never a path inside the project** (§1 row 1) -- every path this
//! module ever opens for writing is under `<state_root>/recovery/records/`.

use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::policy::RecoveryRetentionLimits;

/// The one version this build writes and reads (the same "set a corrupt or future
/// record aside, never guess at it" shape `RUN_RECORD_VERSION` already established).
pub const RECOVERY_RECORD_VERSION: u64 = 1;
const RECOVERY_RECORD_EXTENSION: &str = "json";
const RECOVERY_RECORD_TEMP_EXTENSION: &str = "json.tmp";
/// A record is a buffer's own text plus a small envelope; a file far past the per-record
/// bound is not one this product wrote and is not read whole (the same discipline
/// `RUN_RECORD_MAX_FILE_BYTES` already uses, here sized with headroom over
/// `DEFAULT_RECOVERY_MAX_BYTES_PER_RECORD` for the JSON envelope, since a record that was
/// legitimately written right at the bound must still be readable back).
const RECOVERY_RECORD_MAX_READ_BYTES: u64 = 16 * 1024 * 1024;

/// RFC-027 D2/D13: everything a recovered buffer needs, and nothing it does not --
/// **no undo or redo stack** (D3; §3 row 10 is why the *offer* must say so, not this
/// type's own job). `relative_path` is carried so a reader can tell *which* open document
/// this is without decoding the filename, which is a content-stable hash, not the path
/// itself (a path can contain characters this filesystem layer would rather not carry
/// verbatim, the same reasoning transcript directories are named by id, not by title).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecoveryRecord {
    pub version: u64,
    pub relative_path: String,
    pub text: String,
    pub cursor_line: usize,
    pub cursor_column: usize,
    pub viewport_first_visible_line: usize,
    pub viewport_first_visible_column: usize,
    pub snapshot: RecoveryFileSnapshot,
}

/// A serializable twin of [`crate::content::FileSnapshot`] -- not that type itself,
/// because its own `content_hash` field is documented "not persisted, not a durable file
/// identity," a promise this module keeps by not persisting it: D5's three-way disk
/// comparison (PR-027-C's own job) only ever needs `canonical_path`/`modified_at`/`len`,
/// the same fields `TextDocument::save`'s existing external-change check already treats
/// as what changed, not the hash.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecoveryFileSnapshot {
    pub canonical_path: String,
    pub modified_at_secs: u64,
    pub modified_at_nanos: u32,
    pub len: u64,
}

impl RecoveryFileSnapshot {
    pub fn from_system_time(canonical_path: &Path, modified_at: SystemTime, len: u64) -> Self {
        let since_epoch = modified_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            canonical_path: canonical_path.display().to_string(),
            modified_at_secs: since_epoch.as_secs(),
            modified_at_nanos: since_epoch.subsec_nanos(),
            len,
        }
    }
}

/// RFC-027 D9: a buffer whose own record would exceed the per-record bound. Named so the
/// caller can tell the user *which* buffer and *what* the limit is (§1 row 4: a refusal
/// that does not say so is a protection that silently stopped).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryRecordTooLarge {
    pub relative_path: String,
    pub record_bytes: u64,
    pub max_bytes_per_record: u64,
}

/// `<state_root>/recovery/records/<project_id>/` -- a sibling of `recovery/instances/`
/// (PR-027-A), under the same `recovery/` directory, never inside the project.
pub fn records_dir(state_root: &Path, project_id: &str) -> PathBuf {
    state_root.join("recovery").join("records").join(project_id)
}

/// RFC-027: a record's own filename is a content-stable hash of the document's relative
/// path, not the path itself -- the same document always lands on the same file (so a
/// later write replaces the earlier one rather than accumulating copies), and no path
/// separator, reserved character or length limit from the project's own filesystem ever
/// has to be carried into this one. **Not cryptographic** -- collisions are conceivable at
/// a scale this project does not approach (twenty open documents, D4), and a collision's
/// own cost is bounded: the loser of it is overwritten and simply not offered back, never
/// corrupted or misattributed to another path, since `relative_path` inside the record is
/// what a reader trusts, not the filename.
pub fn record_file_name(relative_path: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    relative_path.hash(&mut hasher);
    format!("{:016x}.{RECOVERY_RECORD_EXTENSION}", hasher.finish())
}

fn record_temp_file_name(relative_path: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    relative_path.hash(&mut hasher);
    format!("{:016x}.{RECOVERY_RECORD_TEMP_EXTENSION}", hasher.finish())
}

/// RFC-027 D13, §2 row 6: writes `record` atomically (a `0600` temporary file in the same
/// `0700` directory, synced, renamed over the target, directory synced) -- the identical
/// durability shape `write_run_record` already uses, with the permissions D13 adds.
/// Refuses (without writing anything) when `record`'s own serialized size would exceed
/// `limits.max_bytes_per_record` (D9, measurement 5): the bound is checked **before** any
/// byte reaches disk, not cleaned up after the fact.
pub fn write_recovery_record(
    state_root: &Path,
    project_id: &str,
    record: &RecoveryRecord,
    limits: RecoveryRetentionLimits,
) -> Result<(), RecoveryRecordWriteError> {
    let mut json = serde_json::to_vec_pretty(record).map_err(RecoveryRecordWriteError::Encode)?;
    json.push(b'\n');

    let record_bytes = json.len() as u64;
    if record_bytes > limits.max_bytes_per_record {
        return Err(RecoveryRecordWriteError::TooLarge(RecoveryRecordTooLarge {
            relative_path: record.relative_path.clone(),
            record_bytes,
            max_bytes_per_record: limits.max_bytes_per_record,
        }));
    }

    // RFC-027 D9: the total bound is app-wide, so a document that already has a record
    // must not be double-counted against its own replacement -- the total excludes this
    // document's own existing record before adding the one about to be written.
    let relative_path_for_total = Path::new(&record.relative_path);
    let existing_bytes = existing_record_bytes(state_root, project_id, relative_path_for_total);
    let total_after_write = total_recovery_record_bytes(state_root)
        .saturating_sub(existing_bytes)
        .saturating_add(record_bytes);
    if total_after_write > limits.max_bytes_total {
        return Err(RecoveryRecordWriteError::TotalBoundExceeded {
            relative_path: record.relative_path.clone(),
            total_bytes_after_write: total_after_write,
            max_bytes_total: limits.max_bytes_total,
        });
    }

    let dir = records_dir(state_root, project_id);
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(RecoveryRecordWriteError::Io)?;
    // `DirBuilder::mode` only governs a directory this call itself creates -- an already
    // existing one (from an earlier run, or a stricter umask) keeps whatever mode it had,
    // so set it explicitly rather than trusting creation to have been the only writer ever.
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
        .map_err(RecoveryRecordWriteError::Io)?;

    let relative_path = Path::new(&record.relative_path);
    let temp_path = dir.join(record_temp_file_name(relative_path));
    if fs::symlink_metadata(&temp_path).is_ok_and(|metadata| metadata.file_type().is_file()) {
        fs::remove_file(&temp_path).map_err(RecoveryRecordWriteError::Io)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp_path)
        .map_err(RecoveryRecordWriteError::Io)?;
    let written = file.write_all(&json).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = written {
        let _ = fs::remove_file(&temp_path);
        return Err(RecoveryRecordWriteError::Io(error));
    }

    let final_path = dir.join(record_file_name(relative_path));
    if let Err(error) = fs::rename(&temp_path, &final_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(RecoveryRecordWriteError::Io(error));
    }
    if let Ok(directory) = File::open(&dir) {
        let _ = directory.sync_all();
    }
    Ok(())
}

#[derive(Debug)]
pub enum RecoveryRecordWriteError {
    Io(io::Error),
    Encode(serde_json::Error),
    TooLarge(RecoveryRecordTooLarge),
    /// RFC-027 D9, measurement 5: writing this record would push the app-wide total
    /// (every project's own records, this document's own previous record excluded) past
    /// `max_bytes_total`. Names the buffer and the limit, the same as `TooLarge`.
    TotalBoundExceeded {
        relative_path: String,
        total_bytes_after_write: u64,
        max_bytes_total: u64,
    },
}

/// RFC-027 D9, §1 row 4: a refusal named by path and reason, not swallowed -- the
/// domain-level shape a UI layer renders from directly, the same "named to the user at
/// the moment it stops being protected" obligation D9 states. Lives in `tekstide-core`,
/// not the shell crate, so a rendering function can depend on it without the shell crate's
/// own message/state types crossing into the render layer -- `RecoveryRecordWriteError`
/// itself is not reused here because its `io::Error`/`serde_json::Error` variants are not
/// `Clone`, and a UI-facing notice needs to be held and compared across frames the same
/// way `SaveAllOutcome` already is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryPersistRefusal {
    pub relative_path: String,
    pub reason: RecoveryPersistRefusalReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryPersistRefusalReason {
    TooLarge {
        record_bytes: u64,
        max_bytes_per_record: u64,
    },
    TotalBoundExceeded {
        total_bytes_after_write: u64,
        max_bytes_total: u64,
    },
    /// A filesystem error, not a bound -- named without detail, since an `io::Error`'s
    /// own text is not guaranteed free of the kind of information this product otherwise
    /// takes care never to echo unescaped, and the actionable fact ("it was not
    /// protected") does not depend on which syscall failed.
    Io,
}

impl From<RecoveryRecordWriteError> for RecoveryPersistRefusalReason {
    fn from(error: RecoveryRecordWriteError) -> Self {
        match error {
            RecoveryRecordWriteError::TooLarge(refusal) => Self::TooLarge {
                record_bytes: refusal.record_bytes,
                max_bytes_per_record: refusal.max_bytes_per_record,
            },
            RecoveryRecordWriteError::TotalBoundExceeded {
                total_bytes_after_write,
                max_bytes_total,
                ..
            } => Self::TotalBoundExceeded {
                total_bytes_after_write,
                max_bytes_total,
            },
            RecoveryRecordWriteError::Io(_) | RecoveryRecordWriteError::Encode(_) => Self::Io,
        }
    }
}

/// RFC-027 D11, measurement 6: the record's own removal -- called when its buffer is
/// saved, closed, or recovered. Missing is not an error: the caller does not have to know
/// whether a record was ever actually written (a clean document, or one under the
/// per-record bound's refusal, has none) to ask for it to be gone.
pub fn remove_recovery_record(
    state_root: &Path,
    project_id: &str,
    relative_path: &Path,
) -> io::Result<()> {
    let path = records_dir(state_root, project_id).join(record_file_name(relative_path));
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// RFC-027: every record file in `project_id`'s own recovery directory, read whole --
/// used by the purge (byte-counting and removal) and, in PR-027-C, by the restart offer.
/// A record this build cannot parse (a future version, or a file this product did not
/// write landing in a directory it owns) is skipped, not deleted and not guessed at --
/// the same "do not delete what you do not recognise" rule PR-027-A's own marker scan
/// already follows.
pub fn read_project_recovery_records(
    state_root: &Path,
    project_id: &str,
) -> Vec<(PathBuf, RecoveryRecord)> {
    let dir = records_dir(state_root, project_id);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(&format!(".{RECOVERY_RECORD_EXTENSION}")))
        })
        .filter_map(|entry| {
            let path = entry.path();
            let record = read_recovery_record_file(&path)?;
            Some((path, record))
        })
        .collect()
}

fn read_recovery_record_file(path: &Path) -> Option<RecoveryRecord> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > RECOVERY_RECORD_MAX_READ_BYTES {
        return None;
    }
    let mut file = File::open(path).ok()?;
    let mut buffer = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut buffer).ok()?;
    let record: RecoveryRecord = serde_json::from_slice(&buffer).ok()?;
    (record.version == RECOVERY_RECORD_VERSION).then_some(record)
}

/// RFC-027 D9, D10: the bytes every record in `project_id`'s own directory holds, and how
/// many -- what a purge would remove (so its own dialog never promises less than it
/// removes) and what `real_retained_recovery_bytes` reports.
pub fn project_recovery_record_bytes(state_root: &Path, project_id: &str) -> (u64, u64) {
    let dir = records_dir(state_root, project_id);
    let Ok(entries) = fs::read_dir(&dir) else {
        return (0, 0);
    };
    let mut count = 0u64;
    let mut bytes = 0u64;
    for entry in entries.flatten() {
        let is_record = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.ends_with(&format!(".{RECOVERY_RECORD_EXTENSION}")));
        if !is_record {
            continue;
        }
        if let Ok(metadata) = fs::symlink_metadata(entry.path())
            && metadata.file_type().is_file()
        {
            count += 1;
            bytes += metadata.len();
        }
    }
    (count, bytes)
}

/// RFC-027 D9: the bytes of `relative_path`'s own existing record in `project_id`, or `0`
/// if it has none -- used to exclude a document's own previous record from the app-wide
/// total before checking whether its replacement fits.
fn existing_record_bytes(state_root: &Path, project_id: &str, relative_path: &Path) -> u64 {
    let path = records_dir(state_root, project_id).join(record_file_name(relative_path));
    fs::symlink_metadata(path)
        .ok()
        .filter(|metadata| metadata.file_type().is_file())
        .map_or(0, |metadata| metadata.len())
}

/// RFC-027 D9, measurement 5: every project's own recovery-record bytes, summed --
/// `<state_root>/recovery/records/*/`. A project directory this call cannot list (not yet
/// created, or genuinely unreadable) contributes nothing rather than failing the whole
/// sum, the same as `project_recovery_record_bytes` already does for one project alone.
fn total_recovery_record_bytes(state_root: &Path) -> u64 {
    let records_root = state_root.join("recovery").join("records");
    let Ok(project_dirs) = fs::read_dir(&records_root) else {
        return 0;
    };
    project_dirs
        .flatten()
        .filter_map(|entry| {
            let project_id = entry.file_name().to_str()?.to_owned();
            Some(project_recovery_record_bytes(state_root, &project_id).1)
        })
        .sum()
}

/// RFC-027 D10: removes every record in `project_id`'s own directory -- the per-project
/// purge's own job (D14: extends the existing purge, does not duplicate it). Regular
/// files only, matched by extension, the same "never a directory, never a symlink, never
/// a stranger's file" discipline `remove_run_record_files` already follows.
pub fn purge_project_recovery_records(state_root: &Path, project_id: &str) -> io::Result<u64> {
    let dir = records_dir(state_root, project_id);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(0);
    };
    let mut removed = 0u64;
    for entry in entries.flatten() {
        let is_record = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.ends_with(&format!(".{RECOVERY_RECORD_EXTENSION}")));
        if !is_record {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !metadata.file_type().is_file() {
            continue;
        }
        match fs::remove_file(entry.path()) {
            Ok(()) => removed += metadata.len(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(relative_path: &str, text: &str) -> RecoveryRecord {
        RecoveryRecord {
            version: RECOVERY_RECORD_VERSION,
            relative_path: relative_path.to_owned(),
            text: text.to_owned(),
            cursor_line: 2,
            cursor_column: 5,
            viewport_first_visible_line: 0,
            viewport_first_visible_column: 0,
            snapshot: RecoveryFileSnapshot::from_system_time(
                Path::new("/project/first.txt"),
                SystemTime::UNIX_EPOCH,
                11,
            ),
        }
    }

    #[test]
    fn record_file_name_is_stable_and_distinct_per_path() {
        let first = record_file_name(Path::new("first.txt"));
        let first_again = record_file_name(Path::new("first.txt"));
        let second = record_file_name(Path::new("second.txt"));
        assert_eq!(
            first, first_again,
            "the same path must always name the same file"
        );
        assert_ne!(
            first, second,
            "different paths must not collide in the common case"
        );
        assert!(first.ends_with(".json"));
    }

    #[test]
    fn record_round_trips_through_json_exactly() {
        let record = sample_record("src/main.rs", "fn main() {}\n");
        let json = serde_json::to_vec(&record).unwrap();
        let back: RecoveryRecord = serde_json::from_slice(&json).unwrap();
        assert_eq!(record, back);
    }

    #[test]
    fn file_snapshot_round_trips_sub_second_precision() {
        let modified_at =
            SystemTime::UNIX_EPOCH + std::time::Duration::new(1_700_000_000, 123_456_789);
        let snapshot =
            RecoveryFileSnapshot::from_system_time(Path::new("/p/f.txt"), modified_at, 42);
        assert_eq!(snapshot.modified_at_secs, 1_700_000_000);
        assert_eq!(snapshot.modified_at_nanos, 123_456_789);
        assert_eq!(snapshot.len, 42);
    }
}
