//! RFC-056 PR-056-B: the record an `AgentRun` leaves in the directory it
//! already owns, `agent-run-<id>/run.json`, beside `transcript.log`.
//!
//! **What it holds** (D8): references and the user's own words — ids,
//! timestamps, the prompt summary, the classification, the notes. Never run
//! content: the transcript is the only place that lives, under its own budget.
//! **Bounded**, and a bounded record says so (`bounds`).
//!
//! **How it is written** (D9, R5): atomically — a temporary file in the same
//! directory, synced, renamed over the record — so a kill mid-write leaves the
//! previous record or none, never half of one. The session writes it when the
//! run *changes*, not when it ends, so an annotation made during a long run
//! survives the app being killed an hour later.
//!
//! **How it is read** (D7, R4): a record that cannot be read, or that names a
//! version this build does not know, is **moved aside** — renamed, never
//! deleted — and reported, and the run then appears as a transcript with no
//! run, never as a run with invented fields. The version is read before
//! anything else, so a newer Tekstide's format cannot be mistaken for
//! corruption of this one's.
//!
//! **What a restored run is** (D6): a record. It carries its ending or says
//! it does not know one, and it lives in a collection no lifecycle consumer
//! reads (`AgentRunOrigin::RestoredFromRecord`).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::{
    AgentCompatibilityLevel, AgentRun, AgentRunId, AgentRunOrigin, AgentRunStatus, ApprovalId,
    AuditEventId, ChangeSetId, DomainTimestamp, RUN_PROMPT_SUMMARY_MAX_CHARS,
    RUN_RECORD_MAX_IDS_PER_KIND, RecordBounds, RunClassification, RunEnding,
};
use crate::project::ProjectId;

pub const RUN_RECORD_FILE_NAME: &str = "run.json";
const RUN_RECORD_TEMP_FILE_NAME: &str = "run.json.tmp";
/// The precedent is `recent-projects.json.corrupt` (`project/recent/store.rs`).
const RUN_RECORD_ASIDE_PREFIX: &str = "run.json.corrupt";
/// The one version this build writes and reads. **A record with any other
/// version is set aside**, so a future format change bumps this and an older
/// Tekstide sets the newer record aside instead of guessing at it (R4).
pub const RUN_RECORD_VERSION: u64 = 1;
/// A record is a few hundred bytes to a few tens of kilobytes at its caps.
/// A file over this is not one this product wrote, and is not read whole.
const RUN_RECORD_MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_ASIDE_SEARCH: u32 = 1_000;

/// Whether `name` is a file in a run directory this product writes and
/// counts as its own: the record, the temporary file a write in progress (or
/// a kill during one) leaves, and a record set aside. The loader uses this
/// so the disk-usage figure never calls the product's own record unclaimed
/// (R1), and **purge deletes by it** (RFC-056 D2), which is why it is exact.
///
/// The set-aside names are the two `move_aside` produces and nothing else:
/// `run.json.corrupt`, and `run.json.corrupt-` followed by one or more ASCII
/// digits. A name that only *begins* like one — `run.json.corruption-notes`,
/// `run.json.corruptXYZ` — is somebody's file, and deleting it would be
/// deleting a file this product never wrote (review 437).
pub fn is_run_record_file_name(name: &str) -> bool {
    if name == RUN_RECORD_FILE_NAME || name == RUN_RECORD_TEMP_FILE_NAME {
        return true;
    }
    let Some(suffix) = name.strip_prefix(RUN_RECORD_ASIDE_PREFIX) else {
        return false;
    };
    suffix.is_empty()
        || suffix
            .strip_prefix('-')
            .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub version: u64,
    pub run_id: String,
    pub project_id: String,
    pub profile_id: String,
    pub prompt_summary: String,
    pub full_prompt_ref: Option<String>,
    pub compatibility_level: String,
    pub status: String,
    pub started_at: Option<String>,
    pub ending: RecordEnding,
    pub classification: Option<RecordClassification>,
    pub notes: Option<String>,
    pub approval_ids: Vec<String>,
    pub change_set_ids: Vec<String>,
    pub audit_event_ids: Vec<String>,
    pub artifact_refs: Vec<String>,
    pub bounds: RecordBoundsFields,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecordEnding {
    /// The run had not ended when this record was written.
    NotEnded,
    Ended {
        at: String,
    },
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecordClassification {
    Coding,
    Review,
    Documentation,
    Testing,
    Refactoring,
    Release,
    Custom { label: String },
}

/// The record's statement of what it left out (D8).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecordBoundsFields {
    pub notes_truncated: bool,
    pub classification_label_truncated: bool,
    pub prompt_summary_truncated: bool,
    pub omitted_approval_ids: u64,
    pub omitted_change_set_ids: u64,
    pub omitted_audit_event_ids: u64,
    pub omitted_artifact_refs: u64,
}

impl RunRecord {
    /// The record for `run`, bounded. **Derived from the run every time**, so
    /// what is written cannot depend on which mutation path changed the run.
    pub fn from_run(run: &AgentRun) -> Self {
        let (prompt_summary, prompt_truncated) =
            bound_text(&run.prompt_summary, RUN_PROMPT_SUMMARY_MAX_CHARS);
        let (approval_ids, approvals_omitted) = bound_list(
            run.approval_ids.iter().map(|id| id.as_str().to_owned()),
            run.approval_ids.len(),
        );
        let (change_set_ids, change_sets_omitted) = bound_list(
            run.change_set_ids.iter().map(|id| id.as_str().to_owned()),
            run.change_set_ids.len(),
        );
        let (audit_event_ids, audit_omitted) = bound_list(
            run.audit_event_ids.iter().map(|id| id.as_str().to_owned()),
            run.audit_event_ids.len(),
        );
        let (artifact_refs, artifacts_omitted) =
            bound_list(run.artifact_refs.iter().cloned(), run.artifact_refs.len());
        let carried = &run.record_bounds;
        Self {
            version: RUN_RECORD_VERSION,
            run_id: run.id.as_str().to_owned(),
            project_id: run.project_id.as_str().to_owned(),
            profile_id: run.profile_id.clone(),
            prompt_summary,
            full_prompt_ref: run.full_prompt_ref.clone(),
            compatibility_level: compatibility_name(run.compatibility_level).to_owned(),
            status: status_name(run.status).to_owned(),
            started_at: run.started_at.as_ref().map(|at| at.as_str().to_owned()),
            ending: match &run.ending {
                RunEnding::NotEnded => RecordEnding::NotEnded,
                RunEnding::Ended(at) => RecordEnding::Ended {
                    at: at.as_str().to_owned(),
                },
                RunEnding::Unknown => RecordEnding::Unknown,
            },
            classification: run.classification.as_ref().map(
                |classification| match classification {
                    RunClassification::Coding => RecordClassification::Coding,
                    RunClassification::Review => RecordClassification::Review,
                    RunClassification::Documentation => RecordClassification::Documentation,
                    RunClassification::Testing => RecordClassification::Testing,
                    RunClassification::Refactoring => RecordClassification::Refactoring,
                    RunClassification::Release => RecordClassification::Release,
                    RunClassification::Custom(label) => RecordClassification::Custom {
                        label: label.clone(),
                    },
                },
            ),
            notes: run.notes.clone(),
            approval_ids,
            change_set_ids,
            audit_event_ids,
            artifact_refs,
            bounds: RecordBoundsFields {
                notes_truncated: carried.notes_truncated,
                classification_label_truncated: carried.classification_label_truncated,
                prompt_summary_truncated: carried.prompt_summary_truncated || prompt_truncated,
                omitted_approval_ids: carried.omitted_approval_ids + approvals_omitted,
                omitted_change_set_ids: carried.omitted_change_set_ids + change_sets_omitted,
                omitted_audit_event_ids: carried.omitted_audit_event_ids + audit_omitted,
                omitted_artifact_refs: carried.omitted_artifact_refs + artifacts_omitted,
            },
        }
    }

    /// The run this record describes, as a **record**: `RestoredFromRecord`,
    /// with a status that claims nothing about a process.
    ///
    /// A record's `expected_*` identity must match where it was found: a
    /// record naming another run or another project is not this run's record.
    /// Every list and text is bounded again on the way in, because the file
    /// is not trusted to have kept to the caps.
    fn into_restored_run(
        self,
        expected_run: &AgentRunId,
        expected_project: &ProjectId,
    ) -> Result<AgentRun, RecordInvalid> {
        if self.run_id != expected_run.as_str() || self.project_id != expected_project.as_str() {
            return Err(RecordInvalid);
        }
        let recorded_status = parse_status(&self.status).ok_or(RecordInvalid)?;
        let compatibility_level = parse_compatibility(&self.compatibility_level)?;
        let started_at = self
            .started_at
            .as_deref()
            .map(|at| DomainTimestamp::from_utc_string(at).map_err(|_| RecordInvalid))
            .transpose()?;

        let (status, ending) = match (recorded_status, &self.ending) {
            // A terminal status carries the ending the record holds, or none:
            // **an ending the record does not hold is unknown**, never the
            // start, never zero, never the moment this file was read (D12).
            (
                AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Cancelled,
                RecordEnding::Ended { at },
            ) => (
                recorded_status,
                RunEnding::Ended(DomainTimestamp::from_utc_string(at).map_err(|_| RecordInvalid)?),
            ),
            (AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Cancelled, _) => {
                (recorded_status, RunEnding::Unknown)
            }
            // Anything else was still going, or awaiting something, when the
            // app closed. Nothing here is supervising it.
            _ => (AgentRunStatus::Detached, RunEnding::Unknown),
        };

        let mut run = AgentRun::draft(
            expected_project.clone(),
            self.profile_id,
            self.prompt_summary,
            compatibility_level,
        );
        run.id = expected_run.clone();
        run.full_prompt_ref = self.full_prompt_ref;
        run.started_at = started_at;
        run.status = status;
        run.ending = ending;
        run.origin = AgentRunOrigin::RestoredFromRecord;

        let mut bounds = RecordBounds {
            notes_truncated: self.bounds.notes_truncated,
            classification_label_truncated: self.bounds.classification_label_truncated,
            prompt_summary_truncated: self.bounds.prompt_summary_truncated,
            omitted_approval_ids: self.bounds.omitted_approval_ids,
            omitted_change_set_ids: self.bounds.omitted_change_set_ids,
            omitted_audit_event_ids: self.bounds.omitted_audit_event_ids,
            omitted_artifact_refs: self.bounds.omitted_artifact_refs,
        };
        let (summary, truncated) = bound_text(&run.prompt_summary, RUN_PROMPT_SUMMARY_MAX_CHARS);
        run.prompt_summary = summary;
        bounds.prompt_summary_truncated |= truncated;

        run.approval_ids = parse_ids(
            self.approval_ids,
            ApprovalId::from_persisted,
            &mut bounds.omitted_approval_ids,
        )?;
        run.change_set_ids = parse_ids(
            self.change_set_ids,
            ChangeSetId::from_persisted,
            &mut bounds.omitted_change_set_ids,
        )?;
        run.audit_event_ids = parse_ids(
            self.audit_event_ids,
            AuditEventId::from_persisted,
            &mut bounds.omitted_audit_event_ids,
        )?;
        let (artifact_refs, omitted) =
            bound_list(self.artifact_refs.iter().cloned(), self.artifact_refs.len());
        run.artifact_refs = artifact_refs;
        bounds.omitted_artifact_refs += omitted;

        run.record_bounds = bounds;
        run.classification = self
            .classification
            .map(|classification| match classification {
                RecordClassification::Coding => RunClassification::Coding,
                RecordClassification::Review => RunClassification::Review,
                RecordClassification::Documentation => RunClassification::Documentation,
                RecordClassification::Testing => RunClassification::Testing,
                RecordClassification::Refactoring => RunClassification::Refactoring,
                RecordClassification::Release => RunClassification::Release,
                RecordClassification::Custom { label } => RunClassification::Custom(label),
            });
        // The setters bound the label and the notes and keep their flags; the
        // flags the file carried are kept beside them.
        let carried = run.record_bounds.clone();
        if let Some(classification) = run.classification.take() {
            run.set_classification(Some(classification))
                .map_err(|_| RecordInvalid)?;
        }
        if let Some(notes) = self.notes {
            run.set_notes(&notes);
        }
        run.record_bounds.notes_truncated |= carried.notes_truncated;
        run.record_bounds.classification_label_truncated |= carried.classification_label_truncated;
        Ok(run)
    }
}

#[derive(Debug)]
struct RecordInvalid;

/// Why a record was set aside.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetAsideReason {
    /// Could not be read as a record: not JSON, half-written, the wrong
    /// shape, over the size cap, or naming another run or project.
    Unreadable,
    /// A well-formed record whose `version` this build does not know.
    UnknownVersion,
}

#[derive(Debug)]
pub enum RunRecordRead {
    /// No record in this run directory.
    Absent,
    Restored(Box<AgentRun>),
    /// The record was not restored. `moved_to` is where it now is; `None`
    /// when it could not be moved (it is then left where it was, and named
    /// again next time).
    SetAside {
        reason: SetAsideReason,
        moved_to: Option<PathBuf>,
    },
}

/// Reads `run_directory/run.json` and restores its run, or sets the record
/// aside. Never deletes: the only mutation is one rename.
pub fn read_run_record(
    run_directory: &Path,
    expected_run: &AgentRunId,
    expected_project: &ProjectId,
) -> RunRecordRead {
    let record_path = run_directory.join(RUN_RECORD_FILE_NAME);
    match fs::symlink_metadata(&record_path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return RunRecordRead::Absent,
        Ok(metadata) if metadata.file_type().is_file() => {}
        // A symlink, a directory or anything else at that name is not a
        // record this product wrote, and is not touched.
        _ => return RunRecordRead::Absent,
    }
    match parse_record(&record_path, expected_run, expected_project) {
        Ok(run) => RunRecordRead::Restored(Box::new(run)),
        Err(reason) => RunRecordRead::SetAside {
            reason,
            moved_to: move_aside(&record_path),
        },
    }
}

fn parse_record(
    record_path: &Path,
    expected_run: &AgentRunId,
    expected_project: &ProjectId,
) -> Result<AgentRun, SetAsideReason> {
    let file = File::open(record_path).map_err(|_| SetAsideReason::Unreadable)?;
    let mut bytes = Vec::new();
    file.take(RUN_RECORD_MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| SetAsideReason::Unreadable)?;
    if bytes.len() as u64 > RUN_RECORD_MAX_FILE_BYTES {
        return Err(SetAsideReason::Unreadable);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| SetAsideReason::Unreadable)?;
    // The version first, on the loosest reading, so a newer format that would
    // not parse as this one is still recognised as *newer* (R4).
    match value.get("version").and_then(Value::as_u64) {
        Some(RUN_RECORD_VERSION) => {}
        Some(_) => return Err(SetAsideReason::UnknownVersion),
        None => return Err(SetAsideReason::Unreadable),
    }
    let record: RunRecord =
        serde_json::from_value(value).map_err(|_| SetAsideReason::Unreadable)?;
    record
        .into_restored_run(expected_run, expected_project)
        .map_err(|_| SetAsideReason::Unreadable)
}

/// `recent-projects.json`'s precedent: rename, never delete, first free name.
fn move_aside(record_path: &Path) -> Option<PathBuf> {
    let directory = record_path.parent()?;
    let first = directory.join(RUN_RECORD_ASIDE_PREFIX);
    let candidates = std::iter::once(first).chain(
        (1..MAX_ASIDE_SEARCH)
            .map(|index| directory.join(format!("{RUN_RECORD_ASIDE_PREFIX}-{index}"))),
    );
    for candidate in candidates {
        if fs::symlink_metadata(&candidate).is_ok() {
            continue;
        }
        return fs::rename(record_path, &candidate).ok().map(|()| candidate);
    }
    None
}

/// Writes `record` atomically into `run_directory`, which must already be a
/// real directory: **the record never creates a run directory**, so a run
/// with no transcript directory gets no record, and a directory purge
/// removed is not brought back by a late write.
pub fn write_run_record(run_directory: &Path, record: &RunRecord) -> io::Result<()> {
    let metadata = fs::symlink_metadata(run_directory)?;
    if !metadata.file_type().is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "a run record is written only into a real run directory",
        ));
    }
    let mut json = serde_json::to_vec_pretty(record).map_err(io::Error::other)?;
    json.push(b'\n');

    let temporary = run_directory.join(RUN_RECORD_TEMP_FILE_NAME);
    // A temporary file an earlier kill left is ours to replace, and only a
    // regular file is: `create_new` below refuses to write through anything
    // planted at that name.
    if fs::symlink_metadata(&temporary).is_ok_and(|metadata| metadata.file_type().is_file()) {
        fs::remove_file(&temporary)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let written = file.write_all(&json).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temporary, run_directory.join(RUN_RECORD_FILE_NAME)) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Ok(directory) = File::open(run_directory) {
        let _ = directory.sync_all();
    }
    Ok(())
}

/// Bytes of the record files in `run_directory`: what a purge would remove
/// beside the transcript, so the purge dialog never promises less than it
/// removes (RFC-056 D2). Exact names, regular files only.
pub fn run_record_bytes(run_directory: &Path) -> u64 {
    record_files_in(run_directory).map(|(_, bytes)| bytes).sum()
}

/// Removes the record files in `run_directory` — `run.json`, `run.json.tmp`
/// and every set-aside name, **matched exactly** and **regular files only** —
/// and returns the bytes removed. Removes nothing else and never a directory:
/// a symlink, a directory or any other name is not this product's to delete.
pub fn remove_run_record_files(run_directory: &Path) -> io::Result<u64> {
    let mut removed = 0;
    for (path, bytes) in record_files_in(run_directory).collect::<Vec<_>>() {
        match fs::remove_file(&path) {
            Ok(()) => removed += bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(removed)
}

fn record_files_in(run_directory: &Path) -> impl Iterator<Item = (PathBuf, u64)> {
    fs::read_dir(run_directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(is_run_record_file_name)
        })
        .filter_map(|entry| {
            let metadata = fs::symlink_metadata(entry.path()).ok()?;
            metadata
                .file_type()
                .is_file()
                .then(|| (entry.path(), metadata.len()))
        })
}

fn bound_text(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() > max_chars {
        (text.chars().take(max_chars).collect(), true)
    } else {
        (text.to_owned(), false)
    }
}

/// The first `RUN_RECORD_MAX_IDS_PER_KIND` items, and how many were left out.
fn bound_list(items: impl Iterator<Item = String>, total: usize) -> (Vec<String>, u64) {
    let kept: Vec<String> = items.take(RUN_RECORD_MAX_IDS_PER_KIND).collect();
    let omitted = total.saturating_sub(kept.len()) as u64;
    (kept, omitted)
}

fn parse_ids<T>(
    ids: Vec<String>,
    parse: impl Fn(String) -> Option<T>,
    omitted: &mut u64,
) -> Result<Vec<T>, RecordInvalid> {
    let total = ids.len();
    let kept = total.min(RUN_RECORD_MAX_IDS_PER_KIND);
    *omitted += (total - kept) as u64;
    ids.into_iter()
        .take(kept)
        .map(|id| parse(id).ok_or(RecordInvalid))
        .collect()
}

fn compatibility_name(level: AgentCompatibilityLevel) -> &'static str {
    match level {
        AgentCompatibilityLevel::Plain => "plain",
        AgentCompatibilityLevel::Supervised => "supervised",
        AgentCompatibilityLevel::Managed => "managed",
    }
}

fn parse_compatibility(name: &str) -> Result<AgentCompatibilityLevel, RecordInvalid> {
    match name {
        "plain" => Ok(AgentCompatibilityLevel::Plain),
        "supervised" => Ok(AgentCompatibilityLevel::Supervised),
        "managed" => Ok(AgentCompatibilityLevel::Managed),
        _ => Err(RecordInvalid),
    }
}

fn status_name(status: AgentRunStatus) -> &'static str {
    match status {
        AgentRunStatus::Draft => "draft",
        AgentRunStatus::Ready => "ready",
        AgentRunStatus::Preparing => "preparing",
        AgentRunStatus::Running => "running",
        AgentRunStatus::AwaitingApproval => "awaiting_approval",
        AgentRunStatus::ReviewReady => "review_ready",
        AgentRunStatus::Completed => "completed",
        AgentRunStatus::Failed => "failed",
        AgentRunStatus::Cancelled => "cancelled",
        AgentRunStatus::Detached => "detached",
    }
}

fn parse_status(name: &str) -> Option<AgentRunStatus> {
    Some(match name {
        "draft" => AgentRunStatus::Draft,
        "ready" => AgentRunStatus::Ready,
        "preparing" => AgentRunStatus::Preparing,
        "running" => AgentRunStatus::Running,
        "awaiting_approval" => AgentRunStatus::AwaitingApproval,
        "review_ready" => AgentRunStatus::ReviewReady,
        "completed" => AgentRunStatus::Completed,
        "failed" => AgentRunStatus::Failed,
        "cancelled" => AgentRunStatus::Cancelled,
        "detached" => AgentRunStatus::Detached,
        _ => return None,
    })
}
