mod availability;
mod state;
mod store;
mod timestamp;

pub use availability::{
    RecentProjectAvailability, RestoredRecentProject, assess_recent_project_availability,
};
pub use state::{RECENT_PROJECT_STATE_VERSION, RecentProject, RecentProjectState};
pub use store::{AppStatePathProvider, RecentProjectStore, RecentProjectStoreError};
pub use timestamp::Timestamp;

#[cfg(test)]
mod tests;
