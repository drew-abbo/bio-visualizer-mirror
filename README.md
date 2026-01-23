# Bio Visualizer

We really need a better name...

---

- [Build Instructions](#build-instructions)
  - [Build Setup](#build-setup)
  - [Packaging a Build for Release](#packaging-a-build-for-release)
- [Development](#development)

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

Linux is not supported just yet.

</td></tr>
</table>

Once the above is satisfied, you're set to build with `cargo`.

### Packaging a Build for Release

To package the app into a self-contained directory/archive, run
[build_package.py](./build_package.py). This script will build all binaries with
the `no-console` feature and the `release-plus` profile, copying all binaries
and non-standard dynamic library dependencies (`.dll`/`.dylib`/`.so` files) into
an output directory/archive.

```sh
python3 ./build_package.py --help
```

## Development

There are 2 binary crates. `launcher` is acts mainly as a project selector for
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
