use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "windows" => windows(),
        "macos" => {}
        "linux" => unimplemented!(),
        _ => panic!("Unsupported target OS `{target_os}`."),
    }
}

fn windows() {
    // This is needed because we need to copy our FFmpeg DLLs from `./ffmpeg`
    // into the target directory so executable can link to them at runtime.

    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");

    let ffmpeg_dir = env::var("FFMPEG_DIR")
        .expect("`FFMPEG_DIR` environment variable unset. Please run `build_setup.py`.");

    let ffmpeg_bin_dir = Path::new(&ffmpeg_dir).join("bin");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target_dir = out_dir.ancestors().nth(3).unwrap();

    for entry in fs::read_dir(&ffmpeg_bin_dir).unwrap() {
        let entry_path = entry.unwrap().path();

        if entry_path.extension().and_then(|s| s.to_str()) == Some("dll") {
            let dll_file_name = entry_path.file_name().unwrap();

            let dest = target_dir.join(dll_file_name);
            fs::copy(&entry_path, &dest).unwrap();

            let dest_deps = target_dir.join("deps").join(dll_file_name);
            fs::copy(&entry_path, &dest_deps).unwrap();
        }
    }
}
