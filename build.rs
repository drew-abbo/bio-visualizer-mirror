use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "windows" => windows(),
        "macos" => unimplemented!(),
        "linux" => unimplemented!(),
        _ => panic!("Unsupported target OS `{target_os}`."),
    }
}

fn windows() {
    // This is needed because we need to copy our FFmpeg DLLs from `./ffmpeg`
    // into the directory where the executable will be generated so that it can
    // link to them at runtime.

    // Rerun if `FFMPEG_DIR` changes.
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let exe_dir = out_dir
        .ancestors()
        .find(|path| {
            let dir_name = path.file_name().unwrap();
            dir_name == "debug" || dir_name == "release"
        })
        .expect("Couldn't find exe directory.");

    let ffmpeg_dir = env::var("FFMPEG_DIR")
        .expect("`FFMPEG_DIR` environment variable unset. Please run `build_setup.py`.");
    let ffmpeg_bin_dir = Path::new(&ffmpeg_dir).join("bin");

    let dlls = fs::read_dir(&ffmpeg_bin_dir)
        .expect("FFmpeg directory missing. Please run `build_setup.py`.")
        .map(|dir_entry| dir_entry.unwrap())
        .filter(|dir_entry| {
            dir_entry
                .path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("dll"))
                .unwrap_or(false)
        })
        .map(|dir_entry| dir_entry.file_name());

    for dll in dlls {
        let dest = Path::new(&exe_dir).join(&dll);
        fs::copy(ffmpeg_bin_dir.join(&dll), &dest).unwrap();
    }
}
