mod launch;
mod profile;
mod report;

pub use launch::{
    AgentAdapterApprovalError, AgentLaunchSummary, AgentRunLaunchPlan, AgentRunLaunchProject,
    AgentRunLaunchRequest, AgentRunLaunchSpec, AgentRunLaunchValidation,
    AgentRunLaunchValidationError, AgentRunLaunchValidator, AgentRunTranscriptCapture,
    AgentRunTranscriptCaptureError, AgentTerminalAdapterApproval, AgentTerminalLaunchIntent,
    VerifiedCwd,
};
pub use profile::{
    AiCliAdapterCapabilities, AiCliEnvironmentPolicy, AiCliExecutable, AiCliExecutableProvenance,
    AiCliProfile, AiCliProfileSource, AiCliPromptPolicy, AiCliWorkspaceDiscoveryPolicy,
    ExecutableLookupPath,
};
pub use report::{
    REPORT_MAX_CHANGED_PATHS, REPORT_TRANSCRIPT_TAIL_BYTES, ReportExportRefusal, RunReportContents,
    RunReportSources, TranscriptExcerpt, render_run_report, run_report_contents, write_run_report,
};

#[cfg(test)]
mod tests;
