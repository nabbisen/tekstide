mod path;
mod policy;
mod reader;
mod retention;
mod writer;

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
    agent_run_may_still_be_writing, is_transcript_expired, mark_transcript_expired_if_due,
    most_recent_activity_seconds,
};
pub use writer::{
    BoundedTranscriptWriter, TranscriptWriteError, TranscriptWriteErrorReason,
    TranscriptWriteSummary, TranscriptWriterConfig,
};

#[cfg(test)]
mod tests;
