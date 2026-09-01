mod backups;
mod clients;
mod links;
mod logs;
mod media_gallery;
mod monitor;
mod watch_group_files;
mod watch_groups;

pub use backups::BackupsPage;
pub use clients::ClientsPage;
pub use links::LinksPage;
pub use logs::LogsPage;
pub use media_gallery::MediaGalleryPage;
pub use monitor::MonitorPage;
pub use watch_group_files::{SortMode, WatchGroupFilesPage};
pub use watch_groups::WatchGroupsPage;
