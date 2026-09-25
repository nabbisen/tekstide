mod loading;
mod path;
mod policy;
mod reader;
mod retention;
mod run_record;
mod writer;

pub(crate) use loading::product_run_directory_of;
pub use loading::{
    FoundTranscriptFile, ProjectTranscriptScan, TranscriptDiskUsage, is_product_run_directory_name,
    scan_project_transcripts, scan_transcript_disk_usage,
};
pub use path::{
    TranscriptPathError, TranscriptPathErrorReason, TranscriptPathRequest, TranscriptPathResolver,
    TranscriptStoragePath,
};
pub use policy::{
    DEFAULT_TRANSCRIPT_MAX_AGE_DAYS, DEFAULT_TRANSCRIPT_MAX_APP_BYTES,
    DEFAULT_TRANSCRIPT_MAX_PROJECT_BYTES, DEFAULT_TRANSCRIPT_MAX_TRANSCRIPT_BYTES,
    TranscriptBudgetScope, TranscriptCaptureMode, TranscriptCapturePolicy,
    TranscriptLocalDataSummary, TranscriptRetentionLimits, TranscriptRetentionState,
};
pub use reader::{
    DEFAULT_TRANSCRIPT_WINDOW_BYTES, TranscriptReadError, TranscriptReadErrorReason,
    TranscriptReadPolicy, TranscriptWindow, read_window,
};
pub use retention::{
    agent_run_may_still_be_writing, clear_stale_expired_mark, is_transcript_expired,
    mark_transcript_expired_if_due, most_recent_activity_seconds,
};
pub use run_record::{
    RUN_RECORD_FILE_NAME, RUN_RECORD_VERSION, RunRecord, RunRecordRead, SetAsideReason,
    is_run_record_file_name, read_run_record, remove_run_record_files, run_record_bytes,
    write_run_record,
};
pub use writer::{
    BoundedTranscriptWriter, TranscriptWriteError, TranscriptWriteErrorReason,
    TranscriptWriteSummary, TranscriptWriterConfig,
};

#[cfg(test)]
mod tests;
