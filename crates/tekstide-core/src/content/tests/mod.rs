mod edit;
mod open;
mod save;

use crate::project::root::{
    ProjectRootHandle, ProjectRootValidator, SymlinkPolicy, ValidProjectRoot,
};
use crate::project::{ProjectId, ProjectSession};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn validate(path: &Path) -> ValidProjectRoot {
    ProjectRootValidator
        .validate(path, SymlinkPolicy::FailClosed)
        .expect("project root should validate")
}

fn root_handle(project_id: ProjectId, root: ValidProjectRoot) -> ProjectRootHandle {
    let project = ProjectSession::new(
        project_id,
        root.display_name,
        root.selected_path,
        root.canonical_path,
    );
    ProjectRootHandle::from_project_session(&project)
}

struct TestSandbox {
    root: PathBuf,
}

impl TestSandbox {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tekstide-{name}-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self { root }
    }

    fn create_dir(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn create_file_with_contents(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
