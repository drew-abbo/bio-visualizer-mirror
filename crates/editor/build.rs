use util::version::APP_NAME;
use util::windows_build::{self, Details};

fn main() {
    windows_build::details()
        .name(format!("{APP_NAME} Editor"))
        .icon("../../logo/s-bg.ico")
        .apply()
        .unwrap();
}
