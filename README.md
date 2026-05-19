# Substrate

---

- [Build Instructions](#build-instructions)
  - [Build Setup](#build-setup)
  - [Packaging a Build for Release](#packaging-a-build-for-release)
- [Development](#development)

## Build Instructions

### Build Setup

1. Ensure you have an up to date version of the Rust toolchain installed (run
   `rustup update`). The project may build on older Rust tooling, but only the
   latest stable versions are guaranteed. If Rust is not yet installed, get it
   from [rustup.rs](https://rustup.rs).
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
<tr><td>Linux</td><td>

Only x86_64 is supported. Tested on Nobara/Fedora 43 and similar
`dnf`-based distros; `apt-get`-based distros (Debian/Ubuntu) should also
work.

`build_setup.py` will prompt to install any missing system packages (via
`sudo dnf` or `sudo apt-get`) and will automatically download FFmpeg 8
pre-built shared libraries from
[BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) into `./ffmpeg/`
(~64 MB). FFmpeg is used at both build-time and runtime.

> **Note:** FFmpeg 8 is not yet available as a system package on most Linux
> distros, which is why `build_setup.py` downloads it locally rather than
> relying on the system FFmpeg.

</td></tr>
</table>

Once the above is satisfied, you're set to build with `cargo`.

### Packaging a Build for Release

To package the app into a self-contained directory/archive, run
[build_package.py](./build_package.py). This script will build all binaries with
the `no-console` feature and by default the `release-plus` profile, moving them
to an output directory/archive (`./package/` by default). It then ensures that
all non-standard dynamic library dependencies the executables need are available
inside the directory/archive.

```sh
python3 ./build_package.py --help
```

## Development

There are 2 binary crates. `launcher` acts mainly as a project selector for
starting up editor instances. `app` is an actual project editor.

Run a binary like this:

```sh
cargo run --bin <BINARY_NAME> -- [ARGUMENTS_FOR_BINARY*]
```

Since both binaries have a UI, both binaries have a `no-console` feature which
ensures a separate console window doesn't start on Windows (this feature just
acts as a no-op on other platforms).

```sh
cargo build --bin <BINARY_NAME> --features no-console
```

There is also an additional build profile `release-plus` that maximizes
optimizations beyond the normal `release` profile (at the cost of debuggability
and compile times).

```sh
cargo build -p <CRATE_NAME> --profile release-plus
```
