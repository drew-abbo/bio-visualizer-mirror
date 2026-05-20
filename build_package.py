#!/usr/bin/env python3

HELP = """
This script compiles and packages the app so that it can be distributed to other
machines (with the same OS and architecture).

The `-y` or `-n` flags can be provided to auto-accept or auto-deny any prompts
for user confirmation.

The `-o` flag is used to specify an output directory to create. The default is
`package`.

The `--no-opt` flag can be used to disable optimization and symbol stripping
(for packaging debug builds). This is really only useful for reducing compile
times.

The `--no-extras` flag skips creating/packaging things like installers or zip
archives.

The `--clean` flag just removes the directory/file specified by `-o` (or its
default).

Make sure to run `build_setup.py` before running this.
""".rstrip()

import sys
import os
import platform
import json
import re
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
    out: str
    no_opt: bool
    no_extras: bool
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
        [-o <OUTPUT_PATH>]
        [--no-opt]
        [--no-extras]
        [--clean]
    {ARG_0} --help
""".rstrip()

    no_opt = False
    no_extras = False
    out = None
    clean = False

    clean_incompatible_arg = None

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

        elif arg == "-o":
            if not (arg := next_arg_or_none()):
                log.fatal(
                    f"Missing argument parameter `<OUTPUT_PATH>` for `{arg}`."
                    + USAGE
                )
            out = arg

        elif arg == "--clean":
            clean = True

        elif arg == "--no-opt":
            no_opt = True
            clean_incompatible_arg = clean_incompatible_arg or arg

        elif arg == "--no-extras":
            no_extras = True
            clean_incompatible_arg = clean_incompatible_arg or arg

        else:
            log.fatal(f"Unknown argument `{arg}`." + USAGE)

    try:
        out = os.path.abspath(out or "package")
    except:
        log.fatal(f"`{out}` is not a valid path.")

    if clean and clean_incompatible_arg:
        log.fatal(
            f"Arguments `{clean_incompatible_arg}` "
            + "and `--clean` are incompatible."
            + USAGE
        )

    return Args(
        out,
        no_opt,
        no_extras,
        clean,
    )


@cache
def app_version() -> str:
    """
    Parses the root `Cargo.toml` file looking for the app's version
    (`workspace.package.version`). The result is cached so that the command only
    ever runs once.
    """

    cargo_toml_path = "./Cargo.toml"

    help_msg = (
        "Ensure `Cargo.toml` is valid and "
        + "`workspace.package.version` is defined."
    )

    try:
        import tomllib
    except ImportError:
        log.warning(
            "Python module `tomllib` unavailable for version query. "
            + "Attempting naive version query..."
        )

        # naive search just looks for lines like `version = "___"`
        try:
            with open(cargo_toml_path, "r") as f:
                version_lines = [
                    line
                    for line in f.readlines()
                    if re.fullmatch(r"\s*version\s*=\s*\"[\w\.\-]+\"\s*", line)
                ]
            if len(version_lines) != 1:
                raise Exception("Expected exactly 1 matching line.")
        except:
            log.fatal("Naive version query failed. " + help_msg)

        line = version_lines[0]
        version = line[line.find('"') + 1 : line.rfind('"')]
    else:
        # The non-naive query actually parses the toml file.
        try:
            with open(cargo_toml_path, "rb") as f:
                cargo_toml = tomllib.load(f)
            version = cargo_toml["workspace"]["package"]["version"]
        except:
            log.fatal("Version query failed. " + help_msg)

    log.info(
        "Found version "
        + f"`{log.Color.INFO}{version}{log.Color.RESET}` in `Cargo.toml`."
    )
    return version


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


def create_out_dir(path: str) -> str:
    """
    Creates an empty directory at `path`. The directory's path is returned.
    """

    try:
        clear_up_path(path)
        os.makedirs(path)
        log.info(f"Output directory created: {path}")
        out_dir = path
    except:
        log.fatal("Failed to create output directory.")

    ends_with_slash = out_dir.endswith(os.sep) or (
        os.altsep and out_dir.endswith(os.altsep)
    )
    if ends_with_slash:
        out_dir = out_dir[:-1]

    return out_dir


def file_name(path: str) -> str:
    """
    The file name from a path.
    """

    return Path(path).name


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
    staging_dir: str,
    *,
    artifact_name_override: Optional[str] = None,
    no_default_features: bool = False,
    features: Optional[list[str]] = None,
    no_opt: bool = False,
    link_time_dir: Optional[str] = None,
    return_dest: bool = False,
    rename: Optional[str] = None,
) -> str:
    """
    Builds a package-ready artifact and copies it into to the provided output
    directory.

    The path of the unstaged artifact (that was copied from) is returned if
    `return_dest` is `false`. Otherwise the staged artifact's path is returned.

    Binaries use the `package-small` profile and dylibs use the `package-fast`
    profile (unless `no_opt` is `True`, in which case the `debug` is used).
    """

    log.info(f"Building artifact for crate `{crate_name}`.")

    crate_kind = get_crate_kind(crate_name)

    if crate_kind == "bin":
        prefix, suffix = "", (".exe" if SYSTEM == "windows" else "")
        profile = "package-small"
    elif crate_kind in ("dylib", "cdylib"):
        if SYSTEM == "windows":
            prefix, suffix = "", ".dll"
        elif SYSTEM == "darwin":  # macOS
            prefix, suffix = "lib", ".dylib"
        elif SYSTEM == "linux":
            prefix, suffix = "", ".so"
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
            env_overrides=(
                {"RUSTFLAGS": f"-L {link_time_dir}"} if link_time_dir else None
            ),
        )
    except sh.CmdException as e:
        log.error(f"{e}")
        log.fatal(
            f"Failed to build `{crate_name}`. "
            + "Ensure `build_setup.py` has been run."
        )

    artifact_name = (
        artifact_name_override
        or f"{prefix}{crate_name.replace("-", "_")}{suffix}"
    )
    target_dir = cargo_metadata()["target_directory"]
    artifact_path = f"{target_dir}/{profile}/{artifact_name}"
    sh.ensure_path_exists(
        artifact_path,
        kind="file",
        help_msg="Cargo built an artifact somewhere unexpected.",
    )

    dest = f"{staging_dir}/{artifact_name if rename is None else rename}"
    try:
        shutil.copy(artifact_path, dest)
    except:
        log.fatal("Failed to copy artifact to output directory.")

    log.info(f"Staged artifact `{artifact_name}`.")

    if return_dest:
        return dest
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


def create_staging_dir(out_dir: str) -> str:
    """
    Creates a directory inside the output directory where the app's contents can
    be staged.
    """

    arch = sh.get_supported_arch()
    if SYSTEM == "windows":
        os_name = "Windows"
    elif SYSTEM == "darwin":
        os_name = "macOS"
    elif SYSTEM == "linux":
        os_name = "Linux"
    else:
        os_name = None
    if arch is None or os_name is None:
        log.fatal("Unsupported system or architecture.")

    staging_dir = f"{out_dir}{os.sep}Substrate-{app_version()}-{os_name}-{arch}"

    try:
        os.makedirs(staging_dir)
    except:
        log.fatal("Failed to create staging directory.")

    return staging_dir


def dump_common_resources(dir: str) -> None:
    """
    Writes/copies resources needed on all platforms into `dir`.
    """

    try:
        # nodes folder
        shutil.copytree("./nodes", f"{dir}/nodes")

        # version file
        with open(f"{dir}/version", "w") as f:
            f.write(app_version())

        # TODO: Also include a license and README file. You can modify the
        # windows installer script to show these to the user.
    except:
        log.fatal(f"Failed to stage resources in `{dir}`.")
    log.info("Common resources staged.")


def windows(out_dir: str, args: Args) -> None:
    """
    Handles Windows-specific packaging steps.
    """

    if sh.get_supported_arch() != "x86_64":
        log.fatal("Windows builds currently only support x86_64.")

    staging_dir = create_staging_dir(out_dir)
    dump_common_resources(staging_dir)

    # Build & stage the app-core DLL.
    unstaged_appcore_dll = build_and_stage_artifact(
        "app-core-dylib",
        staging_dir,
        artifact_name_override="appcore.dll",
        no_opt=args.no_opt,
    )

    # Create a temp dir with the app-core `.lib` file to add to the linker path.
    appcore_lib = f"{unstaged_appcore_dll}.lib"
    sh.ensure_path_exists(appcore_lib, kind="file")
    try:
        temp_appcore_lib_dir = tempfile.mkdtemp()
        shutil.copy(appcore_lib, f"{temp_appcore_lib_dir}\\appcore.lib")
    except:
        log.fatal("Failed to create temporary app-core library directory.")

    def build_and_stage_bin(
        bin_name: str, file_name: str, with_console: bool
    ) -> str:
        return build_and_stage_artifact(
            bin_name,
            staging_dir,
            no_default_features=True,
            features=(
                ["link-dylib"]
                + ([] if with_console else ["no-windows-console"])
            ),
            no_opt=args.no_opt,
            link_time_dir=temp_appcore_lib_dir,
            rename=file_name,
        )

    # Build & stage binaries (a console and non-console version for each).
    for bin_name in ("editor", "launcher"):
        build_and_stage_bin(bin_name, f"{bin_name}-with-console.exe", True)
        build_and_stage_bin(bin_name, f"{bin_name}.exe", False)

    sh.rm_path(temp_appcore_lib_dir)

    # Stage FFmpeg DLLs.
    dlls_copied = sh.copy_files_dir_to_dir(
        ".\\ffmpeg\\bin",
        staging_dir,
        file_ext_filter=".dll",
    )
    log.info(f"Copied {len(dlls_copied)} FFmpeg DLLs into staging directory.")

    log.info("Staging complete.")

    if args.no_extras:
        return

    # Create installer.
    sh.ensure_cmd_exists(
        "iscc",
        help_msg="Inno Setup is required to create an installer. "
        + "Ensure it's installed and the `iscc` command is available.\n"
        + "Inno Setup: https://jrsoftware.org/isinfo.php",
    )
    log.info("Creating installer...")
    try:
        sh.run_cmd(
            "iscc",
            f"/DAppVersion={app_version()}",
            f"/DAppPackagePath={out_dir}",
            f"/O{out_dir}",
            ".\\build_util\\windows-installer.iss",
        )
    except sh.CmdException as e:
        log.fatal(f"{e}")
    log.info("Installer created.")

    # Create a portable zip.
    log.info("Creating portable archive...")
    try:
        shutil.make_archive(staging_dir, "zip", staging_dir)
    except:
        log.fatal(f"Failed to create portable archive.")
    log.info("Portable archive created.")


def mac_os(out_dir: str, args: Args) -> None:
    """
    Handles macOS-specific packaging steps.
    """

    if sh.get_supported_arch() is None:
        log.fatal("macOS builds only support x86_64 and arm64")

    # Ensure Xcode's Command Line Tools are installed.
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

    staging_dir = create_staging_dir(out_dir)
    dump_common_resources(staging_dir)

    appcore_dylib = build_and_stage_artifact(
        "app-core-dylib",
        staging_dir,
        no_opt=args.no_opt,
        return_dest=True,
    )
    appcore_dylib_name = file_name(appcore_dylib)

    sh.run_cmd(
        "install_name_tool",
        "-id",
        f"@loader_path/{appcore_dylib_name}",
        appcore_dylib,
        show_output=False,
    )

    def find_ffmpeg_dylib_dir() -> str:
        log.info("Locating FFmpeg dylib files...")
        sh.ensure_cmd_exists("brew")
        ffmpeg_dir = sh.run_cmd(
            *("brew", "--prefix", "ffmpeg@8"),
            show_output=False,
        )
        ffmpeg_dylib_dir = f"{ffmpeg_dir}/lib"
        sh.ensure_path_exists(ffmpeg_dylib_dir, kind="dir")
        log.info(f"Found FFmpeg dylib files in `{ffmpeg_dylib_dir}`.")
        return ffmpeg_dylib_dir

    def get_dylibs_names(dylib_dir: str, file: str) -> list[str]:
        otool_lines = [
            line.lstrip()
            for line in sh.run_cmd(
                *("otool", "-L", file),
                show_output=False,
            ).splitlines()
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

    ffmpeg_dylib_dir = find_ffmpeg_dylib_dir()

    # Update app-core dylib to point to dylibs in the same local directory
    # instead of pointing at this computer's hard-coded global dylibs.
    dylibs = get_dylibs_names(ffmpeg_dylib_dir, appcore_dylib)
    if len(dylibs) == 0:
        log.warning(f"No FFmpeg dylibs required for `app-core-dylib`.")
    else:
        log.info(f"Remapping {len(dylibs)} FFmpeg dylibs for `app-core-dylib`.")
        for dylib in dylibs:
            dylib_src_path = f"{ffmpeg_dylib_dir}/{dylib}"
            stage_dylib(dylib_src_path)
            sh.run_cmd(
                "install_name_tool",
                "-change",
                dylib_src_path,
                f"@loader_path/{dylib}",
                appcore_dylib,
                show_output=False,
            )

    # Create a temp dir with the app-core dylib file to add to the linker path.
    try:
        temp_app_lib_dir = tempfile.mkdtemp()
        shutil.copy(appcore_dylib, temp_app_lib_dir)
    except:
        log.fatal("Failed to create temporary app-core library directory.")

    for bin_name in ("editor", "launcher"):
        bin_path = build_and_stage_artifact(
            bin_name,
            staging_dir,
            no_default_features=True,
            features=["link-dylib"],
            no_opt=args.no_opt,
            link_time_dir=temp_app_lib_dir,
            return_dest=True,
        )

        sh.run_cmd(
            "install_name_tool",
            "-change",
            f"@rpath/{appcore_dylib_name}",
            f"@executable_path/{appcore_dylib_name}",
            bin_path,
            show_output=False,
        )

    sh.rm_path(temp_app_lib_dir)


def main() -> None:
    start_time = time.time()

    args = parse_args()

    sh.require_script_in_working_dir()

    if args.clean:
        if sh.rm_path(args.out, allow_missing=True):
            log.success(f"Removed `{args.out}`.")
        else:
            log.success("Nothing changed.")
        return

    out_dir = create_out_dir(args.out)

    sh.ensure_cmd_exists("cargo")

    if SYSTEM == "windows":
        windows(out_dir, args)
    elif SYSTEM == "darwin":  # macOS
        mac_os(out_dir, args)
    elif SYSTEM == "linux":
        log.fatal("Linux support is currently unimplemented.")
    else:
        log.fatal(f"Unsupported system: `{SYSTEM}`")

    elapsed_time = time.time() - start_time
    log.success(
        f"Packaging completed in {fmt_time(elapsed_time)}. "
        + f"See directory: {args.out}"
    )


if __name__ == "__main__":
    sh.catch_stop_signal(main)
