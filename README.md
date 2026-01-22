# Bio Vizualizer

We really need a better name...

## Build Instructions

1. Ensure you have an up to date version of the Rust toolchain installed (run
   `rustup update`). The project may build on older Rust tooling, but only the
   latest stable versions are guaranteed.
2. Ensure you have Python 3.9 or newer.
3. Ensure you have the necessary additional dependencies for your platform (see
   table below).
4. Run `build_setup.py`. This will walk you through any steps you need to take
   before you can build. Run this script until it says you're all set (you may
   need to run it multiple times if you're missing dependencies).

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
</table>

Once the above is satisfied, you're set to build with `cargo`.

You can build a binary with maximum optimizations (ready for release) like this
(note that binaries may rely on local shared libraries):

```sh
cargo build -p <PKG> --profile release-plus --features --no-console
```

- The `release-plus` profile can be enabled to maximize optimizations (at the
  cost of debuggability and compile times).
- The `no-console` feature can be enabled for binaries to disable the console
  that pops up when you run on Windows.
