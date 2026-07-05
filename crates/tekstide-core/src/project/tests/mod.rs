use super::{ProjectId, ProjectSession};

mod collections;
mod metadata;
mod references;

fn project_session(sequence: u64) -> ProjectSession {
    ProjectSession::new(
        ProjectId::for_test(sequence),
        format!("Project {sequence}"),
        format!("/workspace/project-{sequence}"),
        format!("/workspace/project-{sequence}"),
    )
}
