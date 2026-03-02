pub struct AppSaveContext {
    pub has_edits: bool,
}

impl AppSaveContext {
    pub fn new() -> Self {
        Self { has_edits: false }
    }

    pub fn pre_close_event() {
        
    }
}