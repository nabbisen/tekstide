//! RFC-056 PR-056-B: a project's `AgentRun`s and the records they leave.
//!
//! A child of `session`, so it reaches the session's own collections without
//! widening any of them. **Every lifecycle consumer reads `agent_runs`, and
//! nothing restored is ever in it** (D6): a run restored from its record
//! cannot be counted against the launch limit, block change attribution,
//! read as running or failed, or hold the close prompt open, because it is
//! not in any collection those read.

use std::path::{Path, PathBuf};

use crate::domain::{
    AgentRun, AgentRunId, AgentRunOrigin, BlankClassificationLabel, OwnershipError,
    RunClassification,
};
use crate::transcript::{
    RunRecord, RunRecordRead, SetAsideReason, read_run_record, write_run_record,
};

use super::{ProjectSession, TranscriptLoadSummary};

/// What writing one run's record did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunRecordWrite {
    Written,
    /// The record on disk already says this. Nothing was written.
    Unchanged,
    /// The run has no run directory — no transcript was captured, or its
    /// transcript was purged — and **a record never creates one**. The change
    /// lives in memory until the app closes.
    NoRunDirectory,
    /// The write failed. The next attempt tries again.
    Failed,
}

/// RFC-056 D7: records this session did not restore.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RunRecordsSetAside {
    /// Could not be read; renamed with a `.corrupt` name, never deleted.
    pub unreadable: u64,
    /// Written by a version this build does not know; renamed the same way.
    pub unknown_version: u64,
    /// Could not be read **and could not be renamed**: still `run.json`.
    pub left_in_place: u64,
}

impl RunRecordsSetAside {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// What one pass over a project's runs wrote.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RunRecordPersistSummary {
    pub written: u64,
    pub unchanged: u64,
    pub no_run_directory: u64,
    pub failed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunAnnotationError {
    Ownership(OwnershipError),
    BlankClassificationLabel,
}

impl From<OwnershipError> for RunAnnotationError {
    fn from(error: OwnershipError) -> Self {
        Self::Ownership(error)
    }
}

impl From<BlankClassificationLabel> for RunAnnotationError {
    fn from(_: BlankClassificationLabel) -> Self {
        Self::BlankClassificationLabel
    }
}

impl ProjectSession {
    /// Runs restored from their records. **Records, not lifecycles**: a run
    /// here is never running, never counts against a limit, and never owns a
    /// change.
    pub fn restored_agent_runs(&self) -> &[AgentRun] {
        &self.restored_agent_runs
    }

    /// RFC-056 D7: what this session set aside, for the Project Board to name.
    /// Accumulated across loads and never reset: it is a fact about this
    /// session's state directory, not about the last call.
    pub fn run_records_set_aside(&self) -> RunRecordsSetAside {
        self.run_records_set_aside
    }

    /// The run to show as this project's latest: the last one launched here,
    /// or, when none was, the last one restored.
    pub fn latest_agent_run_for_display(&self) -> Option<&AgentRun> {
        self.agent_runs
            .last()
            .or_else(|| self.restored_agent_runs.last())
    }

    /// Any run, launched here or restored.
    pub fn agent_run_or_restored(&self, agent_run_id: &AgentRunId) -> Option<&AgentRun> {
        self.agent_runs
            .iter()
            .chain(self.restored_agent_runs.iter())
            .find(|run| run.id == *agent_run_id)
    }

    /// RFC-056 D3. The classification is persisted **at once**, not at the
    /// next pass, so a run killed a moment later keeps it (D9).
    pub fn set_agent_run_classification(
        &mut self,
        agent_run_id: &AgentRunId,
        classification: Option<RunClassification>,
    ) -> Result<RunRecordWrite, RunAnnotationError> {
        self.annotated_run_mut(agent_run_id)?
            .set_classification(classification)?;
        Ok(self.persist_run_record(agent_run_id))
    }

    /// RFC-056 D4. Nothing but the user's text reaches this; see `AgentRun`.
    pub fn set_agent_run_notes(
        &mut self,
        agent_run_id: &AgentRunId,
        notes: &str,
    ) -> Result<RunRecordWrite, RunAnnotationError> {
        self.annotated_run_mut(agent_run_id)?.set_notes(notes);
        Ok(self.persist_run_record(agent_run_id))
    }

    /// Writes the record of every run whose record is not what the run now
    /// says. **The record is derived from the run each time and compared to
    /// what was last written**, so "every change" holds whichever path
    /// changed the run, and an untouched run costs a comparison, not a write.
    pub fn persist_agent_run_records(&mut self) -> RunRecordPersistSummary {
        let ids: Vec<AgentRunId> = self
            .agent_runs
            .iter()
            .chain(self.restored_agent_runs.iter())
            .map(|run| run.id.clone())
            .collect();
        let mut summary = RunRecordPersistSummary::default();
        for id in ids {
            match self.persist_run_record(&id) {
                RunRecordWrite::Written => summary.written += 1,
                RunRecordWrite::Unchanged => summary.unchanged += 1,
                RunRecordWrite::NoRunDirectory => summary.no_run_directory += 1,
                RunRecordWrite::Failed => summary.failed += 1,
            }
        }
        summary
    }

    fn annotated_run_mut(
        &mut self,
        agent_run_id: &AgentRunId,
    ) -> Result<&mut AgentRun, OwnershipError> {
        self.agent_runs
            .iter_mut()
            .chain(self.restored_agent_runs.iter_mut())
            .find(|run| run.id == *agent_run_id)
            .ok_or(OwnershipError::MissingReference)
    }

    fn persist_run_record(&mut self, agent_run_id: &AgentRunId) -> RunRecordWrite {
        let Some(run) = self.agent_run_or_restored(agent_run_id) else {
            return RunRecordWrite::NoRunDirectory;
        };
        let record = RunRecord::from_run(run);
        if self.run_records_written.get(agent_run_id) == Some(&record) {
            return RunRecordWrite::Unchanged;
        }
        let Some(directory) = self.run_directory_of(run) else {
            return RunRecordWrite::NoRunDirectory;
        };
        match write_run_record(&directory, &record) {
            Ok(()) => {
                self.run_records_written
                    .insert(agent_run_id.clone(), record);
                RunRecordWrite::Written
            }
            Err(_) => RunRecordWrite::Failed,
        }
    }

    /// The run's directory, from the transcript it owns, or the directory its
    /// record was found in. **None once that transcript is a tombstone**: a
    /// purged run's record is not written back.
    fn run_directory_of(&self, run: &AgentRun) -> Option<PathBuf> {
        match run.origin {
            AgentRunOrigin::LaunchedHere => self
                .transcripts
                .iter()
                .find(|transcript| transcript.agent_run_id() == Some(&run.id))
                .filter(|transcript| !transcript.is_tombstone())
                .and_then(|transcript| transcript.storage_path.parent())
                .map(Path::to_path_buf),
            AgentRunOrigin::RestoredFromRecord => {
                let attached_transcript_purged = run
                    .transcript_ref
                    .as_ref()
                    .and_then(|id| self.transcripts.iter().find(|t| &t.id == id))
                    .is_some_and(|transcript| transcript.is_tombstone());
                if attached_transcript_purged {
                    None
                } else {
                    self.restored_run_directories.get(&run.id).cloned()
                }
            }
        }
    }

    /// Restores the runs whose records the scan found, and sets aside the ones
    /// that cannot be. Called by `load_transcripts_from_disk`, after its
    /// transcripts are in, so a restored run's transcript can be attached.
    pub(super) fn restore_runs_from_records(
        &mut self,
        record_directories: &[PathBuf],
        summary: &mut TranscriptLoadSummary,
    ) {
        for directory in record_directories {
            let Some(run_id) = directory
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| AgentRunId::from_persisted(name.to_owned()))
            else {
                continue;
            };
            if self.agent_run_or_restored(&run_id).is_some() {
                continue;
            }
            match read_run_record(directory, &run_id, &self.id) {
                RunRecordRead::Absent => {}
                RunRecordRead::Restored(run) => {
                    let mut run = *run;
                    run.transcript_ref = self
                        .transcripts
                        .iter()
                        .find(|transcript| transcript.storage_path.parent() == Some(directory))
                        .map(|transcript| transcript.id.clone());
                    self.run_records_written
                        .insert(run.id.clone(), RunRecord::from_run(&run));
                    self.restored_run_directories
                        .insert(run.id.clone(), directory.clone());
                    self.restored_agent_runs.push(run);
                    summary.run_records_restored += 1;
                }
                RunRecordRead::SetAside { moved_to: None, .. } => {
                    // Not moved: it is still `run.json`, and the next open
                    // meets it again. Said as what it is, not as "set aside".
                    summary.run_records_left_in_place += 1;
                    self.run_records_set_aside.left_in_place += 1;
                }
                RunRecordRead::SetAside { reason, .. } => match reason {
                    SetAsideReason::Unreadable => {
                        summary.run_records_set_aside_unreadable += 1;
                        self.run_records_set_aside.unreadable += 1;
                    }
                    SetAsideReason::UnknownVersion => {
                        summary.run_records_set_aside_unknown_version += 1;
                        self.run_records_set_aside.unknown_version += 1;
                    }
                },
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn agent_run_mut_for_test(&mut self, agent_run_id: &AgentRunId) -> &mut AgentRun {
        self.annotated_run_mut(agent_run_id).unwrap()
    }
}
