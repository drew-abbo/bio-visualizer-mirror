![Substrate](./logo/full-bg-3072x1024.png)

- [Build Instructions](#build-instructions)
  - [Build Setup](#build-setup)
  - [Packaging a Build for Release](#packaging-a-build-for-release)
- [Development](#development)
  - [Binary Crates](#binary-crates)
  - [Core Wrapper Crates](#core-wrapper-crates)
  - [Non-Core Library Crates](#non-core-library-crates)
  - [Versioning](#versioning)

## Build Instructions

### Build Setup

1. Ensure you have an up to date version of the Rust toolchain installed (run
   `rustup update`). The project may build on older Rust tooling, but only the
   latest stable versions are guaranteed.
2. Ensure you have Python 3.9 or newer.
3. Ensure you have the necessary additional dependencies for your platform (see
   table below).
4. Run [build_setup.py](./build_setup.py). This will walk you through any steps
   you need to take before you can build. Run this script until it says you're
   all set (you may need to run it multiple times if you're missing
   dependencies).

```sh
python3 ./build_setup.py --help
```

<table>
<tr><th>Platform</th><th>Details</th></tr>
</tr><td>Windows</td><td>

Only Windows 11 (x86_64) is supported. The project may be able to build on
Windows 10, but it is not being intentionally supported.

- Ensure you're using the `x86_64-pc-windows-msvc` toolchain for Rust (the
  default).
- Ensure you have the
  [Visual Studio Installer](https://visualstudio.microsoft.com/downloads/) (2022
  or 2026, *Community* is fine).
- The [7z command-line utility](https://www.7-zip.org/download.html) is
  optional, but it may make the build setup process easier if you already have
  it.

</td></tr>
<tr><td>MacOS</td><td>

Only MacOS Monterey and newer is supported. The project may be able to build on
older versions, but it is not being intentionally supported. Both x86_64 and
Arm64 (Apple silicon) platforms are natively supported.

</td></tr>

</td></tr>
<tr><td>Linux</td><td>

Linux is not officially supported just yet.

</td></tr>
</table>

Once the above is satisfied, you're set to build with `cargo`:

```sh
cargo build -p editor && cargo run -p launcher
```

### Packaging a Build for Release

To package the app into a self-contained directory/archive, run
[build_package.py](./build_package.py). This script will build everything for
your platform and move it to a self-contained directory/archive (`./package/` by
default).

```sh
python3 ./build_package.py --help
```

## Development

### Binary Crates

There are 2 binary crates. `launcher` acts mainly as a project selector for
starting up editor instances. `editor` is an actual project editor.

Run a binary like this:

```sh
cargo run --bin <BINARY_NAME> -- [ARGUMENTS_FOR_BINARY*]
```

### Core Wrapper Crates

The `editor` and `launcher` binary crates are really just wrappers around the
`editor-core` and `launcher-core` library crates (or the `app-core` crate when
the `link-dylib` feature is enabled).

This is done to enable the `link-dylib` feature for the `editor` and `launcher`
binary crates. When this feature is enabled (and the `link-static` feature is
disabled), the binaries will expect to be able to link to a dynamic library
called `app_core_dylib` (e.g. `app_core_dylib.dll` on Windows,
`app_core_dylib.dylib` on Unix). This dynamic library re-exports the same
things `editor-core` and `launcher-core` export, just through a C-ABI. Building
the `app-core-dylib` crate will create this shared library.

Doing dynamic linking like this move's all of the app's code into the shared
library, leaving the binaries as just thin wrappers. This makes it more
reasonable to ship many different binaries since each one doesn't need to come
with everything statically linked (making file sizes huge). For example, on
Windows we ship 4 different executables (a console and no-console variation of
both binaries).

To reduce compilation times, dynamic linking is not enabled by default.

### Non-Core Library Crates

- The `engine` crate is a library for handling node graphs and rendering.
- The `media` crate is a library for handling media data (images, video, MIDI).
- The `util` crate is a library of common useful utilities that can be used
  across any of the other crates. Each utility is gated behind a feature.

### Versioning

The app's version is set by the `version` field in the root
[Cargo.toml](./Cargo.toml) file.
