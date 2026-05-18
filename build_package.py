#!/usr/bin/env python3

HELP = """
This script compiles and packages the app so that it can be distributed to other
machines (with the same OS and architecture).

The `-y` or `-n` flags can be provided to auto-accept or auto-deny any prompts
for user confirmation.

The `--no-opt` flag can be used to disable optimization and symbol stripping
(for packaging debug builds). This is really only useful for reducing compile
times.

When the `-o` flag is provided, a file extension can also be provided so that an
archive is created instead of a directory. For example `-o out` will create a
directory, but `-o out.zip` or `-o out.tar.xz` will create an archive. The
default (if `-o` is not provided) is to create a directory called `package`.

The `--clean` flag just removes the directory/file specified by `-o` (or its
default).

Make sure to run `build_setup.py` before running this.
""".rstrip()

import sys
import os
import platform
import json
import time
import shutil
import tempfile
from functools import cache
from dataclasses import dataclass
from pathlib import Path
from typing import TypedDict, Optional, Union

import build_util.log as log
import build_util.sh as sh
import build_util.user as user

SYSTEM = platform.system().lower()


@dataclass
class Args:
    no_opt: str
    out: str
    clean: bool


def parse_args() -> Args:
    """
    Parses command line arguments.
    """

    ARG_0 = sys.argv[0]
    USAGE = f"""
Usage:
    {ARG_0}
        [-y|-n]
        [--no-opt]
        [-o <OUTPUT_PATH>[.zip|.tar|.tar.gz|.tar.bz|.tar.xz]]
        [--clean]
    {ARG_0} --help
""".rstrip()

    no_opt = False
    out = None
    clean = False

    args = iter(sys.argv[1:])
    seen_args: set[str] = set()

    def next_arg_or_none() -> Optional[str]:
        try:
            arg = next(args)
        except StopIteration:
            return None

        if arg in seen_args:
            log.fatal(f"Repeat argument `{arg}`." + USAGE)
        seen_args.add(arg)

        return arg

    auto_confirm = None

    while (arg := next_arg_or_none()) is not None:
        if arg in ("-h", "--help", "help", "/h", "/?", "h", "?"):
            print(f"{USAGE}\n{HELP}".rstrip())
            sys.exit(int(len(sys.argv) != 2))

        if arg in ("-y", "-n"):
            if auto_confirm:
                log.fatal(
                    f"Arguments `{arg}` and `-{auto_confirm}`"
                    + " are incompatible."
                    + USAGE
                )
            auto_confirm = arg[1]
            user.set_confirm_auto_answer(auto_confirm)

        elif arg == "--clean":
            clean = True

        elif arg == "-o":
            if not (arg := next_arg_or_none()):
                log.fatal(
                    f"Missing argument parameter `<OUTPUT_PATH>` for `{arg}`."
                    + USAGE
                )
            out = arg

        elif arg == "--no-opt":
            no_opt = True

        else:
            log.fatal(f"Unknown argument `{arg}`." + USAGE)

    try:
        out = os.path.abspath(out or "package")
    except:
        log.fatal(f"`{out}` is not a valid path.")

    if no_opt and clean:
        log.fatal(
            f"Arguments `--no-opt` and `--clean` are incompatible." + USAGE
        )

    return Args(no_opt, out, clean)


def clear_up_path(path: str) -> None:
    """
    If there is an object at the provided path, the user is asked to move/remove
    it (with an option to have it removed automatically). If this function
    returns, the path has been cleared.
    """

    if not os.path.exists(path):
        return

    if not user.confirm(f"An object already exists at `{path}`. Remove it?"):
        if os.path.exists(path):
            log.fatal(
                f"Can't continue while an object at `{path}` still exists."
            )
        else:
            log.info(f"Object at `{path}` has moved. Continuing...")

    if sh.rm_path(path, allow_missing=True):
        log.warning(f"Removed `{path}`.")
    else:
        log.info(f"Nothing to remove anymore at `{path}`.")


def create_staging_dir(path: Optional[str]) -> str:
    """
    Creates an empty staging directory at `path` if it's provided (emptying it
    if it exists) or in a temporary location. The directory's path is returned.
    """

    try:
        if path is not None:
            clear_up_path(path)
            os.makedirs(path)
            log.info(f"Staging directory created (`{path}`).")
            staging_dir = path
        else:
            staging_dir = tempfile.mkdtemp()
            log.info(f"Temporary staging directory created (`{staging_dir}`).")
    except:
        log.fatal("Failed to initialize staging directory.")

    ends_with_slash = staging_dir.endswith(os.sep) or (
        os.altsep and staging_dir.endswith(os.altsep)
    )
    if ends_with_slash:
        staging_dir = staging_dir[:-1]

    return staging_dir


def file_name(path: str) -> str:
    """
    The file name from a path.
    """

    return Path(path).name


def file_ext(path: str) -> Optional[str]:
    """
    The file extension of a path.
    """

    out_name = file_name(path)
    return out_name.split(".", 1)[-1] if "." in out_name else None


def get_archive_fmt(
    path: str, ask_on_unknown_ext: bool = True
) -> Optional[str]:
    """
    The archive format to use for `shutil.make_archive` to make a file with the
    same extension as `path`. `None` is returned if the extension isn't
    recognized or is missing.
    """

    ext = file_ext(path)
    if ext == "zip":
        return "zip"
    if ext == "tar":
        return "tar"
    if ext == "tar.gz":
        return "gztar"
    if ext == "tar.bz":
        return "bztar"
    if ext == "tar.xz":
        return "xztar"

    if (
        ext is not None
        and ask_on_unknown_ext
        and not user.confirm(
            f"Unsupported archive format `.{ext}`. "
            + "Would you like to create a directory?"
        )
    ):
        log.fatal(f"Unsupported archive format `.{ext}`.")

    return None


def try_archive(
    out_file: str,
    src_dir: str,
) -> str:
    """
    Tries to archive `src_dir` into `out_file` and remove `src_dir`. If creating
    an archive fails, the user will be asked if it's okay to create a directory
    instead. The path of the archive or directory is returned. The return value
    will match `out_file` if and only if creating the archive succeeded.
    """

    archive_fmt = get_archive_fmt(out_file, ask_on_unknown_ext=False)
    ext = file_ext(out_file)
    assert ext is not None and archive_fmt is not None

    out_path_without_ext = out_file[: -(len(ext) + 1)]

    try:
        clear_up_path(out_file)
        log.info("Archiving output.")
        shutil.make_archive(out_path_without_ext, archive_fmt, src_dir)
    except:
        err_msg = f"Failed to create `.{ext}` archive."
        if user.confirm(
            f"{err_msg} A directory `{out_path_without_ext}`"
            + " can be created instead. Would you rather exit?"
        ):
            log.fatal(err_msg)

        clear_up_path(out_path_without_ext)
        try:
            shutil.move(src_dir, out_path_without_ext)
        except:
            log.fatal(f"Failed to move staging directory.")
        return out_path_without_ext

    sh.rm_path(src_dir)

    return out_file


class CargoTarget(TypedDict):
    kind: list[str]


class CargoPackage(TypedDict):
    name: str
    targets: list[CargoTarget]


class CargoMetadata(TypedDict):
    packages: list[CargoPackage]
    target_directory: str


@cache
def cargo_metadata() -> CargoMetadata:
    """
    Returns metadata from Cargo. The result is cached so that the command only
    ever runs once.
    """

    return json.loads(
        sh.run_cmd(
            "cargo",
            "metadata",
            "--no-deps",
            "--offline",
            "--quiet",
            "--format-version",
            "1",
            show_output=False,
        )
    )


@cache
def get_crate_kind(crate_name: str) -> str:
    """
    Returns the kind of a crate by looking at the metadata from Cargo (e.g.
    `bin`, `lib`, `cdylib`).
    """

    for package in cargo_metadata()["packages"]:
        if package["name"] == crate_name:
            return package["targets"][0]["kind"][0]
    log.fatal(f"Unknown crate `{crate_name}`.")


def build_and_stage_artifact(
    crate_name: str,
    out_dir: str,
    *,
    no_default_features: bool = False,
    features: Optional[list[str]] = None,
    no_opt: bool = False,
    rustflags: Optional[str] = None,
) -> str:
    """
    Builds a package-ready artifact and copies it into to the provided output
    directory. The path of the unstaged artifact (that was copied from) is
    returned.

    Binaries use the `package-small` profile and dylibs use the `package-fast`
    profile (unless `no_opt` is `True`, in which case the `debug` is used).
    """

    log.info(f"Building artifact for crate `{crate_name}`.")

    crate_kind = get_crate_kind(crate_name)

    if crate_kind == "bin":
        ext = ".exe" if SYSTEM == "windows" else ""
        profile = "package-small"
    elif crate_kind in ("dylib", "cdylib"):
        if SYSTEM == "windows":
            ext = ".dll"
        elif SYSTEM == "darwin":  # MacOS
            ext = ".dylib"
        elif SYSTEM == "linux":
            ext = ".so"
        profile = "package-fast"
    else:
        log.fatal(f"Unexpected crate kind  `{crate_kind}` for `{crate_name}`.")

    if no_opt:
        profile = "debug"
        profile_args = []
    else:
        profile_args = ["--profile", profile]

    features_args = []
    if no_default_features:
        features_args.append("--no-default-features")
    if features is not None and len(features) > 0:
        features_args.extend(("--features", " ".join(features)))

    try:
        sh.run_cmd(
            *(
                *("cargo", "build"),
                *("-p", crate_name),
                *profile_args,
                *features_args,
                *("--color", "always" if log.Color.ENABLED else "never"),
            ),
            non_fatal=True,
            env_overrides={"RUSTFLAGS": rustflags} if rustflags else None,
        )
    except sh.CmdException as e:
        log.warning(f"{e}")
        log.fatal(
            f"Failed to build `{crate_name}`. "
            + "Ensure `build_setup.py` has been run."
        )

    target_dir = cargo_metadata()["target_directory"]
    artifact_name = f"{crate_name.replace("-", "_")}{ext}"
    artifact_path = f"{target_dir}/{profile}/{artifact_name}"
    sh.ensure_path_exists(
        artifact_path,
        kind="file",
        help_msg="Cargo built an artifact somewhere unexpected.",
    )

    try:
        shutil.copy(artifact_path, out_dir)
    except:
        log.fatal("Failed to copy artifact to output directory.")

    log.info(f"Staged artifact `{artifact_name}`.")

    return artifact_path


def fmt_time(secs: float) -> str:
    """
    Formats a time to be human readable (e.g. `"1 minute and 15 seconds"`).
    """

    hours, sub_hour_secs = divmod(int(secs), 3600)
    mins, secs = sub_hour_secs // 60, (sub_hour_secs % 60) + (secs - int(secs))

    def pluralize(noun: str, n: Union[int, float]) -> str:
        return f"{noun}{'s' if n < 0.95 or n >= 1.05 else ''}"

    hours_str = f"{hours} {pluralize('hour', hours)}"
    mins_str = f"{mins} {pluralize('min', mins)}"
    secs_str = (
        f"{int(secs)} " if round(secs, 1).is_integer() else f"{secs:.1f} "
    ) + pluralize("second", secs)

    if hours:
        if mins:
            return f"{hours_str}, {mins_str}, and {secs_str}"
        return f"{hours_str} and {secs_str}"
    if mins:
        return f"{mins_str} and {secs_str}"
    return secs_str


def windows(staging_dir: str, no_opt: bool) -> None:
    """
    Handles Windows-specific packaging steps.
    """

    if sh.get_supported_arch() != "x86_64":
        log.fatal("Windows builds currently only support x86_64.")

    app_core_dll = build_and_stage_artifact(
        "app-core-dylib",
        staging_dir,
        no_opt=no_opt,
    )

    # Create a temp dir with the `.lib` file to add to the linker path
    app_core_lib = f"{app_core_dll}.lib"  # "app_core_dylib.dll.lib"
    sh.ensure_path_exists(app_core_lib, kind="file")
    try:
        temp_app_lib_dir = tempfile.mkdtemp()
        shutil.copy(app_core_lib, f"{temp_app_lib_dir}\\app_core_dylib.lib")
    except:
        log.fatal("Failed to create temporary app-core library directory")

    def build_and_stage_bin(bin_name: str, *, with_console: bool) -> str:
        return build_and_stage_artifact(
            bin_name,
            staging_dir,
            no_default_features=True,
            features=(
                ["link-dylib"]
                + ([] if with_console else ["no-windows-console"])
            ),
            no_opt=no_opt,
            rustflags=f"-L {temp_app_lib_dir}",
        )

    for bin_name in ("editor", "launcher"):
        # Create both console and non-console binaries on windows
        build_and_stage_bin(bin_name, with_console=True)
        try:
            shutil.move(
                f"{staging_dir}\\{bin_name}.exe",
                f"{staging_dir}\\{bin_name}-with-console.exe",
            )
        except:
            log.fatal("Failed to rename executable.")
        build_and_stage_bin(bin_name, with_console=False)

    sh.rm_path(temp_app_lib_dir)

    # Stage FFmpeg DLLs.
    dlls_copied = sh.copy_files_dir_to_dir(
        ".\\ffmpeg\\bin",
        staging_dir,
        file_ext_filter=".dll",
    )
    log.info(f"Copied {len(dlls_copied)} FFmpeg DLLs into staging directory.")


def mac_os(staging_dir: str) -> None:
    """
    Handles MacOS-specific packaging steps.
    """

    log.fatal("MacOS support is currently broken.")

    def ensure_cli_tools_installed() -> None:
        try:
            sh.ensure_cmd_exists("otool", non_fatal=True)
            sh.ensure_cmd_exists("install_name_tool", non_fatal=True)
        except sh.DoesntExistException:
            if not user.confirm(
                "It doesn't look like Xcode's Command Line Tools are installed."
                + " Would you like to install them?"
            ):
                log.fatal("Xcode's Command Line Tools are required.")

            sh.ensure_cmd_exists("xcode-select")
            sh.run_cmd("xcode-select", "--install")

            sh.ensure_cmd_exists("otool")
            sh.ensure_cmd_exists("install_name_tool")

    def find_ffmpeg_dylib_dir() -> str:
        log.info("Locating FFmpeg dylib files.")
        sh.ensure_cmd_exists("brew")
        ffmpeg_dylib_dir = f"{sh.run_cmd('brew', '--prefix', 'ffmpeg@8')}/lib"
        sh.ensure_path_exists(ffmpeg_dylib_dir, kind="dir")
        log.info(f"Found FFmpeg dylib files in `{ffmpeg_dylib_dir}`.")
        return ffmpeg_dylib_dir

    def get_dylibs_names(dylib_dir: str, file: str) -> list[str]:
        otool_lines = [
            line.lstrip()
            for line in sh.run_cmd("otool", "-L", file).splitlines()
        ]
        return [
            line[len(dylib_dir) + 1 :].split(" ")[0]
            for line in otool_lines
            if line.startswith(dylib_dir)
        ]

    def stage_dylib(dylib_path: str) -> None:
        src = os.path.realpath(dylib_path)
        dest = f"{staging_dir}/{file_name(dylib_path)}"
        try:
            shutil.copy(src, dest)
        except:
            log.fatal(f"Failed to copy `{src}` to `{dest}`.")

    ensure_cli_tools_installed()
    ffmpeg_dylib_dir = find_ffmpeg_dylib_dir()

    app_core = f"app_core_dylib.dylib"

    dylibs = get_dylibs_names(ffmpeg_dylib_dir, f"{staging_dir}/{app_core}")
    if len(dylibs) == 0:
        log.warning(f"No FFmpeg dylibs required for `{app_core}`")

    log.info(f"Remapping {len(dylibs)} FFmpeg dylibs for `{app_core}`.")
    for dylib in dylibs:
        dylib_src_path = f"{ffmpeg_dylib_dir}/{dylib}"
        stage_dylib(dylib_src_path)
        sh.run_cmd(
            "install_name_tool",
            "-change",
            dylib_src_path,
            f"@executable_path/{dylib}",
            f"{staging_dir}/{app_core}",
            show_output=False,
        )


def main() -> None:
    start_time = time.time()

    args = parse_args()

    if args.clean:
        if sh.rm_path(args.out, allow_missing=True):
            log.success(f"Removed `{args.out}`.")
        else:
            log.success("Nothing changed.")
        return

    archive_fmt = get_archive_fmt(args.out)
    user_wants_archive = archive_fmt is not None
    staging_dir = create_staging_dir(None if user_wants_archive else args.out)

    sh.ensure_cmd_exists("cargo")

    if SYSTEM == "windows":
        windows(staging_dir, args.no_opt)
    elif SYSTEM == "darwin":  # MacOS
        mac_os(staging_dir)
    elif SYSTEM == "linux":
        log.fatal("Linux support is currently unimplemented.")
    else:
        log.fatal(f"Unsupported system: `{SYSTEM}`")

    if not user_wants_archive:
        out_path = args.out
        archive_was_made = False
    else:
        out_path = try_archive(args.out, staging_dir)
        archive_was_made = out_path == args.out
    out_kind = "archive" if archive_was_made else "directory"

    elapsed_time = time.time() - start_time
    log.success(
        f"Packaging completed in {fmt_time(elapsed_time)}. "
        + f"See {out_kind}: {out_path}"
    )


if __name__ == "__main__":
    sh.catch_stop_signal(main)
