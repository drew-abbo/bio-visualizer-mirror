use std::path::PathBuf;

#[derive(Debug)]
pub enum UiToMedia {
    LoadVideo(PathBuf),
    ExtractAllFrames,
    Shutdown,
}

#[derive(Debug)]
pub enum MediaToUi {
    Status(String),
    Progress { done: usize, total: usize },
    Finished,
    Error(String),
}