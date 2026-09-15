//! RFC-050 PR-050-B: reading back the transcripts earlier runs left on disk.
//!
//! **Whatever this module accepts, purge and retention can delete**, so the
//! rules for what it accepts are the security surface
//! (`what-loading-a-transcript-must-not-do.md`):
//!
//! - **no symlink is followed at any level**, and every check uses
//!   `symlink_metadata`, never `metadata`;
//! - only a **regular file named `transcript.log`, exactly two levels under
//!   `transcripts/`** is a transcript;
//! - a run directory must be named in **this product's own spelling**,
//!   `agent-run-` and a lowercase hyphenated UUID;
//! - **anything else is skipped and never deleted**, and its bytes are counted
//!   so a user can see them.
//!
//! **Nothing here deletes.** Records built from a scan are purged through
//! `ProjectSession::purge_transcript_at`, like launched ones (§3).

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::project::ProjectId;

const TRANSCRIPTS_DIRECTORY: &str = "transcripts";
const TRANSCRIPT_FILE_NAME: &str = "transcript.log";
const RUN_DIRECTORY_PREFIX: &str = "agent-run-";
/// Bounds the byte count of an unrecognised tree. Nothing this product writes
/// is deeper than two levels, so anything past this is not ours to measure.
const MAX_TREE_DEPTH: usize = 8;

/// One `transcript.log` a scan accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoundTranscriptFile {
    pub path: PathBuf,
    /// Read once, at the scan. `None` only when the filesystem will not say.
    pub modified_unix_seconds: Option<u64>,
    /// Whether another handle held the file's exclusive lock at the one
    /// probe (RFC-050 D3). A held lock means live for the whole session.
    pub writer_held_lock: bool,
}

/// What a scan of one project's transcript directory found.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectTranscriptScan {
    pub found: Vec<FoundTranscriptFile>,
    /// Entries skipped because they are not a transcript this product wrote.
    pub skipped_entries: u64,
    /// Their bytes, counted without following any symlink.
    pub skipped_bytes: u64,
}

/// Bytes under the whole `transcripts/` directory (RFC-050 D5, D6′).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TranscriptDiskUsage {
    /// Everything under `transcripts/`, including projects that are closed.
    pub total_bytes: u64,
    /// Bytes no open or recent project claims, plus unrecognised entries
    /// inside the directories that are claimed.
    pub unclaimed_bytes: u64,
}

/// A run directory is loaded only when its name is exactly what this product
/// writes: `agent-run-` followed by a **lowercase hyphenated** UUID, checked as
/// a round trip.
///
/// `AgentRunId::from_persisted` is deliberately not used: it inherits
/// `uuid::Uuid::parse_str`'s acceptance of uppercase, hyphen-less, braced and
/// `urn:uuid:` spellings, none of which this product ever writes. Tightening
/// `from_persisted` itself would change what `recent-projects.json` and the
/// audit store accept (response 388).
pub fn is_product_run_directory_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix(RUN_DIRECTORY_PREFIX) else {
        return false;
    };
    uuid::Uuid::parse_str(suffix).is_ok_and(|uuid| uuid.hyphenated().to_string() == suffix)
}

/// Enumerates `<state_root>/transcripts/<project_id>/` and probes each accepted
/// file's lock once.
///
/// The state root is a parameter, resolved by the caller through the same
/// split the launch uses, so **no test can reach the real state root** (§5).
/// It is canonicalised once, as the launch's resolver does, so a found path
/// compares equal to the path a launch in this session recorded.
pub fn scan_project_transcripts(
    state_root: &Path,
    project_id: &ProjectId,
) -> ProjectTranscriptScan {
    let Ok(state_root) = fs::canonicalize(state_root) else {
        return ProjectTranscriptScan::default();
    };
    scan_project_directory(&state_root, project_id, true)
}

/// Bytes under all of `transcripts/`, split into what `claimed` projects own
/// and what nothing owns. Reads sizes only: it creates no records and probes
/// no locks.
pub fn scan_transcript_disk_usage(state_root: &Path, claimed: &[ProjectId]) -> TranscriptDiskUsage {
    let mut usage = TranscriptDiskUsage::default();
    let Ok(state_root) = fs::canonicalize(state_root) else {
        return usage;
    };
    let Some(transcripts) = real_directory(&state_root.join(TRANSCRIPTS_DIRECTORY)) else {
        return usage;
    };
    let Ok(entries) = fs::read_dir(&transcripts) else {
        return usage;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let bytes = tree_bytes(&path, 0);
        usage.total_bytes += bytes;
        let claimed_project = entry
            .file_name()
            .to_str()
            .and_then(|name| claimed.iter().find(|id| id.as_str() == name));
        match claimed_project {
            Some(project_id) if real_directory(&path).is_some() => {
                usage.unclaimed_bytes +=
                    scan_project_directory(&state_root, project_id, false).skipped_bytes;
            }
            _ => usage.unclaimed_bytes += bytes,
        }
    }
    usage
}

fn scan_project_directory(
    state_root: &Path,
    project_id: &ProjectId,
    probe_locks: bool,
) -> ProjectTranscriptScan {
    let mut scan = ProjectTranscriptScan::default();
    let Some(transcripts) = real_directory(&state_root.join(TRANSCRIPTS_DIRECTORY)) else {
        return scan;
    };
    let Some(project_directory) = real_directory(&transcripts.join(project_id.as_str())) else {
        return scan;
    };
    let Ok(entries) = fs::read_dir(&project_directory) else {
        return scan;
    };

    for entry in entries.flatten() {
        let run_directory = entry.path();
        let named_as_ours = entry
            .file_name()
            .to_str()
            .is_some_and(is_product_run_directory_name);
        if !named_as_ours || real_directory(&run_directory).is_none() {
            skip(&mut scan, &run_directory);
            continue;
        }

        let Ok(run_entries) = fs::read_dir(&run_directory) else {
            continue;
        };
        for run_entry in run_entries.flatten() {
            let path = run_entry.path();
            let is_transcript_name = run_entry.file_name() == TRANSCRIPT_FILE_NAME;
            match fs::symlink_metadata(&path) {
                Ok(metadata) if is_transcript_name && metadata.file_type().is_file() => {
                    let modified_unix_seconds = metadata
                        .modified()
                        .ok()
                        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                        .map(|elapsed| elapsed.as_secs());
                    let writer_held_lock = probe_locks && lock_is_held(&path);
                    scan.found.push(FoundTranscriptFile {
                        path,
                        modified_unix_seconds,
                        writer_held_lock,
                    });
                }
                _ => skip(&mut scan, &path),
            }
        }
    }
    scan
}

/// RFC-050 D3: the one probe. Takes the lock and **drops it at once** by
/// dropping the handle before returning. A lock that cannot be taken for any
/// reason — including a file that cannot be opened — reads as held, because
/// unknown liveness is live (RFC-049 §1).
fn lock_is_held(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return true;
    };
    let held = file.try_lock().is_err();
    drop(file);
    held
}

/// `Some(path)` only for a real directory: `symlink_metadata`, so a symlink to
/// a directory is not one.
fn real_directory(path: &Path) -> Option<PathBuf> {
    let metadata = fs::symlink_metadata(path).ok()?;
    metadata.file_type().is_dir().then(|| path.to_path_buf())
}

fn skip(scan: &mut ProjectTranscriptScan, path: &Path) {
    scan.skipped_entries += 1;
    scan.skipped_bytes += tree_bytes(path, 0);
}

/// Bytes under `path` **without following symlinks**: a symlink counts as
/// nothing, because what it points at is not in the state root.
fn tree_bytes(path: &Path, depth: usize) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    let file_type = metadata.file_type();
    if file_type.is_file() {
        return metadata.len();
    }
    if !file_type.is_dir() || depth >= MAX_TREE_DEPTH {
        return 0;
    }
    fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| tree_bytes(&entry.path(), depth + 1))
                .sum()
        })
        .unwrap_or(0)
}
