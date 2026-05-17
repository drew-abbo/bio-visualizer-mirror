use util::version::APP_NAME;
use util::windows_exe::{self, Details};

fn main() {
    windows_exe::details()
        .name(format!("{APP_NAME} (Launcher)"))
        .icon("../../logo/desktop-ico.ico")
        .apply()
        .unwrap();
}
