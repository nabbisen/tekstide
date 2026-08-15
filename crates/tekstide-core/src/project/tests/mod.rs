use super::{ProjectId, ProjectSession};

mod change_detection;
mod collections;
mod content;
mod metadata;
mod references;
mod transcripts;

fn project_session(sequence: u64) -> ProjectSession {
    ProjectSession::new(
        ProjectId::for_test(sequence),
        format!("Project {sequence}"),
        format!("/workspace/project-{sequence}"),
        format!("/workspace/project-{sequence}"),
    )
}
