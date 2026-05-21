use util::version::APP_NAME;
use util::windows_build::{self, Details};

fn main() {
    windows_build::details()
        .name(format!("{APP_NAME} App Core Dylib"))
        .apply()
        .unwrap();
}
