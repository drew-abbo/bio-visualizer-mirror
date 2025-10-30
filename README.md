# Build Instructions

For now, only Windows 11 (x86_64) is supported.

Make sure you've installed the Rust toolchain (using rustup) with a version of
at least 1.90.0. Ensure you're using the `x86_64-pc-windows-msvc` toolchain.
You'll also need the Visual Studio Installer.

Before you try and build, run `python3 ./build_setup.py`. This will walk you
through any steps you need to take before you can build. Run this script until
it says you're all set. You may need to run it multiple times.

Run `cargo build`. If this works, you can run the program with `cargo run`,
passing arguments after a `--` argument (e.g. `cargo run -- arg1 arg2`). You can
also see documentation with `cargo doc --open`.
