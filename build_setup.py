#!/usr/bin/env python3

# This is nasty but it's the nicest solution I could come up with. Run this
# before you try and run `cargo build`. Follow the instructions (possibly
# re-running it a few times) until it says you're done.
#
# This script:
# - Ensures you have the proper C compiler dependencies to build ffmpeg-next's
#   rust bindings.
# - Ensures you have FFmpeg shared libraries/headers (with an option to
#   download them automatically).
# - Generates a `.cargo/config.toml` that sets all the needed environment
#   variables to build ffmpeg-next.
#
# Usage:
#     build_setup.py [-y]
#
# On Windows, the compiled executable may depend on shared library files (DLLs).
# These should be placed in the same directory as the executable.

import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import typing
from typing import Literal, NoReturn, Iterable, Sequence, Any, Union


output_is_terminal = sys.stdout.isatty()


class Color:
    ERROR = "\033[31m" if output_is_terminal else ""
    WARNING = "\033[33m" if output_is_terminal else ""
    INFO = "\033[36m" if output_is_terminal else ""
    SUCCESS = "\033[32m" if output_is_terminal else ""
    CONFIRM = "\033[35m" if output_is_terminal else ""
    ACTION_NEEDED = "\033[35m\033[1m" if output_is_terminal else ""
    COMMAND = "\033[34m" if output_is_terminal else ""
    RESET = "\033[0m" if output_is_terminal else ""


# Print an error and exit.
def fatal(
    *args: Any,
    include_run_again_msg: bool = True,
    sep: Union[str, None] = " ",
) -> NoReturn:
    print(f"{Color.ERROR}FATAL{Color.RESET}: ", end="", file=sys.stderr)
    print(*args, sep=sep, file=sys.stderr)
    if include_run_again_msg:
        print(
            "\nPlease run this script again once the issue is resolved.",
            file=sys.stderr,
        )
    sys.exit(1)


# Print a warning.
def warning(
    *args: Any,
    sep: Union[str, None] = " ",
    end: Union[str, None] = "\n",
    flush: bool = False,
) -> None:
    print(
        f"{Color.WARNING}WARNING{Color.RESET}: ",
        end="",
        file=sys.stderr,
        flush=False,
    )
    print(*args, sep=sep, file=sys.stderr, end=end, flush=flush)


# Print some info.
def info(
    *args: Any,
    sep: Union[str, None] = " ",
    end: Union[str, None] = "\n",
    flush: bool = False,
) -> None:
    print(f"{Color.INFO}INFO{Color.RESET}: ", end="", flush=False)
    print(*args, sep=sep, end=end, flush=flush)


# Print that the process is done (success).
def success(
    *args: Any,
    sep: Union[str, None] = " ",
    end: Union[str, None] = "\n",
    flush: bool = False,
) -> None:
    print(f"{Color.SUCCESS}SUCCESS{Color.RESET}: ", end="", flush=False)
    print(*args, sep=sep, end=end, flush=flush)


# When set, all `confirm` confirmations with be responded to with this instead
# of prompting the user.
confirm_auto_answer = None


# Print a message and await a "yes"/"no" from the user.
def confirm(*args: Any, sep: Union[str, None] = " ") -> bool:
    print(f"{Color.CONFIRM}CONFIRM{Color.RESET}: ", end="")
    print(*args, sep=sep, end="")
    print(f" ({Color.CONFIRM}y{Color.RESET}/n): {Color.CONFIRM}", end="")

    if confirm_auto_answer is None:
        response = input().strip().lower()
        print(f"{Color.RESET}", end="", flush=True)
    else:
        response = confirm_auto_answer
        print(f"{confirm_auto_answer}{Color.RESET} (auto)")

    return response in ("y", "yes")


# Print a message and wait for the user to hit enter.
def action_needed(*args: Any, sep: Union[str, None] = " ") -> None:
    print(f"{Color.ACTION_NEEDED}MANUAL ACTION NEEDED{Color.RESET}: ", end="")
    print(*args, sep=sep, end="")
    print(
        f" (press [{Color.ACTION_NEEDED}ENTER{Color.RESET}] if you have "
        + "completed the action manually or enter a shell command to run): "
        + f"{Color.ACTION_NEEDED}",
        end="",
    )

    response = input()
    if not output_is_terminal:
        print(f"{response}{Color.RESET} (auto)")
    print(f"{Color.RESET}", end="", flush=True)

    if len(response) != 0 and not response.isspace():
        run_cmd(response, shell=True)


# Parses command line arguments.
def parse_args() -> None:
    arg0 = sys.argv[0]
    for arg in sys.argv[1:]:
        if arg == "-y":
            global confirm_auto_answer
            confirm_auto_answer = "y"
        else:
            fatal(f"Unknown argument `{arg}`.\n" + f"Usage: {arg0} [-y]")


def get_supported_arch() -> Union[Literal["x86_64", "arm64"], None]:
    return typing.cast(
        Union[Literal["x86_64", "arm64"], None],
        {
            "x86_64": "x86_64",
            "amd64": "x86_64",
            "arm64": "arm64",
        }.get(platform.machine().lower()),
    )


# Does a check to see if a path exists.
def ensure_path_exists(
    path: str, help_msg: Union[str, None] = None, non_fatal: bool = False
) -> None:
    if not os.path.exists(path):
        err_msg = f"Couldn't find `{path}`." + (
            f"\n{help_msg}" if help_msg is not None else ""
        )
        if non_fatal:
            raise DoesntExistException(err_msg)
        fatal(err_msg)


# Does a check to see if a command exists on the `PATH`.
def ensure_cmd_exists(
    cmd: str, help_msg: Union[str, None] = None, non_fatal: bool = False
) -> None:
    if shutil.which(cmd) is None:
        err_msg = f"Couldn't find `{cmd}` on the path." + (
            f"\n{help_msg}" if help_msg is not None else ""
        )
        if non_fatal:
            raise DoesntExistException(err_msg)
        fatal(err_msg)


# Raised if something goes wrong running a command.
class CmdException(Exception):
    pass


class DoesntExistException(Exception):
    pass


# Joins the command arguments into a single string, naively wrapping arguments
# that contain spaces in double quotes.
def format_cmd(cmd: Iterable[str]) -> str:
    return " ".join(arg if " " not in arg else f'"{arg}"' for arg in cmd)


def print_running_cmd(cmd: Sequence[str]) -> None:
    # Highlight the file name in the first argument.
    last_slash_idx = max(cmd[0].rfind("/"), cmd[0].rfind("\\"))
    highlight_start_idx = 0 if last_slash_idx == -1 else last_slash_idx + 1

    cmd = [
        f"{cmd[0][:highlight_start_idx]}"
        + f"{Color.COMMAND}{cmd[0][highlight_start_idx:]}{Color.RESET}",
        *cmd[1:],
    ]

    print(f"{Color.COMMAND}RUNNING COMMAND{Color.RESET}: `{format_cmd(cmd)}`")


# Runs a shell command and returns its output (minus a trailing newline if it
# has one).
def run_cmd(*cmd: str, shell: bool = False, non_fatal: bool = False) -> str:
    print_running_cmd(cmd)
    print(f"{Color.COMMAND}{' OUTPUT ':~^80}{Color.RESET}", flush=True)

    try:
        process = subprocess.Popen(
            cmd if not shell else " ".join(cmd),
            shell=shell,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,  # Combine stderr and stdout to stdout.
            text=True,
            bufsize=1,  # Line buffering.
        )

        # Capture and print lines as they come in.
        output = ""
        if process.stdout is not None:
            for line in process.stdout:
                output += line
                print(line, end="", flush=True)
        process.wait()

    except KeyboardInterrupt:
        raise
    finally:
        print(f"\n{Color.RESET + Color.COMMAND}{'~' * 80}{Color.RESET}")

    if (exit_code := process.returncode) != 0:
        err_msg = f"`{format_cmd(cmd)}` failed with exit code {exit_code}."
        if non_fatal:
            raise CmdException(err_msg)
        fatal(err_msg)

    return output[:-1] if output.endswith("\n") else output


# Like `run_cmd` except it doesn't wait for the command to finish.
def start_cmd(*cmd: str, shell: bool = False) -> None:
    print_running_cmd(cmd)
    subprocess.Popen(cmd if not shell else " ".join(cmd), shell=shell)


# Create a `.cargo/config.toml` file.
def create_cargo_config(contents: str) -> None:
    path = (
        ".cargo\\config.toml"
        if platform.system() == "Windows"
        else ".cargo/config.toml"
    )

    if os.path.exists(path) and not confirm(
        f"A `{path}` file already exists. Overwrite?"
    ):
        fatal(f"Failed to create `{path}`.")
    else:
        os.makedirs(".cargo", exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write("# Generated by `build_setup.py`.\n" + contents)

    info(f"Cargo config generated (`{path}`).")


# Handles build setup for Windows builds.
def windows() -> None:
    if get_supported_arch() != "x86_64":
        fatal("Windows builds currently only support x86_64.")

    program_files = os.environ.get("ProgramFiles(x86)")
    if program_files is None:
        fatal("Coldn't find ProgramFiles(x86).")

    vs_installer_dir = f"{program_files}\\Microsoft Visual Studio\\Installer"

    ensure_path_exists(
        vs_installer_dir,
        help_msg="You likely don't have `Visual Studio Installer`"
        + " on your system. Please install it from here:\n"
        + "https://visualstudio.microsoft.com/",
    )

    # `vswhere` lets us find where a specific version is installed.
    ensure_path_exists(f"{vs_installer_dir}\\vswhere.exe")
    try:
        vs_installation_path = run_cmd(
            f"{vs_installer_dir}\\vswhere.exe",
            "-property",
            "installationPath",
            "-version",
            "[17.0, 19.0)",  # Only Visual Studio 2022 or 2026.
            "-latest",
            non_fatal=True,
        )
    except CmdException:
        fatal("Couldn't find Visual Studio (2022 or 2026).")
    info("MSVC found.")

    # Collect a list of all Visual Studio components that are installed.
    def get_installed_vs_components() -> list[str]:
        info("Exporting installed Visual Studio components...")

        # We'll use `vs_installer` to query installed components. The result
        # will be written to a JSON config file we can read.

        temp_vs_installer_config_path = (
            f"{tempfile.gettempdir()}\\config.vsconfig"
        )

        vs_installer_err_msg = (
            "Failed to export installed Visual Studio components."
            + " Make sure the Visual Studio Installer isn't already runnning."
        )

        try:
            run_cmd(
                f"{vs_installer_dir}\\vs_installer.exe",
                "--installPath",
                vs_installation_path,
                "export",
                "--config",
                temp_vs_installer_config_path,
                "--quiet",
                "--noUpdateInstaller",
                non_fatal=True,
            )
        except CmdException as e:
            fatal(f"{e}\n{vs_installer_err_msg}")

        try:
            with open(temp_vs_installer_config_path) as f:
                installed_components = json.load(f)["components"]
            os.remove(temp_vs_installer_config_path)
        except FileNotFoundError:
            fatal(vs_installer_err_msg)

        info("Installed Visual Studio components exported.")
        return installed_components

    installed_vs_components = get_installed_vs_components()

    # Ensures we have a required Visual Studio component installed.
    def ensure_vs_component_is_installed(
        component_name: str, component_regex: str
    ) -> None:
        component_is_installed = any(
            re.fullmatch(component_regex, installed_component) is not None
            for installed_component in installed_vs_components
        )

        if not component_is_installed:
            start_cmd(f"{vs_installer_dir}\\vs_installer.exe")
            fatal(
                f"Missing the `{component_name}` component."
                + " Please use the Visual Studio Installer to install it.\n"
                + "Trying to opening the Visual Studio Installer..."
            )

        info(f"The `{component_name}` Visual Studio component is installed.")

    # At a minimum, rust needs C++ build tools and a Windows 11 SDK.
    ensure_vs_component_is_installed(
        "C++ x64/x86 build tools (Latest)",
        r"Microsoft\.VisualStudio\.Component\.VC\.Tools\..*",
    )
    ensure_vs_component_is_installed(
        "Windows 11 SDK",
        r"Microsoft\.VisualStudio\.Component\.Windows11SDK\..*",
    )

    # `ffmpeg-next` uses `bindgen` which requires `libclang` to generate rust
    # bindings. We get that from this component.
    ensure_vs_component_is_installed(
        "C++ Clang Compiler for Windows",
        r"Microsoft\.VisualStudio\.Component\.VC\.Llvm\.Clang",
    )

    # Finds the `libclang` DLL.
    def get_libclang_path() -> str:
        # `libclang` needs to be installed for FFmpeg-next to be able to create
        # rust bindings.
        libclang_path = f"{vs_installation_path}\\VC\\Tools\\LLVM\\x64\\bin"
        ensure_path_exists(f"{libclang_path}\\libclang.dll")
        info("Found `libclang`.")

        return libclang_path

    # Try and ensure we're using the right header files for generating rust
    # bindings for FFmpeg.
    def try_to_get_clang_include_dir() -> Union[str, None]:
        try:
            clang_dir = (
                f"{vs_installation_path}\\VC\\Tools\\LLVM\\x64\\lib\\clang"
            )

            newest_clang_version = sorted(
                version
                for version in os.listdir(clang_dir)
                if os.path.isdir(os.path.join(clang_dir, version))
            )[-1]

            clang_include_dir = f"{clang_dir}\\{newest_clang_version}\\include"
            ensure_path_exists(clang_include_dir, non_fatal=True)

            info("Found Clang include directory.")
        except (FileNotFoundError, IndexError, DoesntExistException):
            warning(
                "Failed to find Clang include directory. Compilation may fail."
            )
            clang_include_dir = None

        return clang_include_dir

    # Make sure we have FFmpeg installed in the project directory.
    def get_ffmpeg_dir() -> str:
        FFMPEG_ZIP_PATH = ".\\ffmpeg.7z"
        FFMPEG_DIR_LOCAL = ".\\ffmpeg"
        ffmpeg_dir = os.path.abspath(FFMPEG_DIR_LOCAL)

        ffmpeg_dir_exists = os.path.exists(ffmpeg_dir)

        # If they've already got a downloaded zip file for it then we can just
        # tell them to extract it. We can't extract it for them because the
        # download is a 7z file for some reason and there's no way to extract a
        # 7z file on windows without using the file explorer UI or downloading
        # an external utility. The external utility we could download (7za)
        # comes as a zipped .7z file so we can't even do that automatically.
        if os.path.exists(FFMPEG_ZIP_PATH) and not ffmpeg_dir_exists:
            info("Attempting to open file explorer on FFmpeg zip file.")
            start_cmd(
                "explorer",
                "/select,",
                f"{os.path.abspath(FFMPEG_ZIP_PATH)}",
            )
            action_needed(
                f"Please extract `{FFMPEG_ZIP_PATH}`"
                + f" to `{FFMPEG_DIR_LOCAL}`.",
            )

            ffmpeg_dir_exists = os.path.exists(ffmpeg_dir)
            if not ffmpeg_dir_exists:
                fatal("The FFmpeg directory wasn't extracted.")

        if ffmpeg_dir_exists:
            # If there's only 1 item in the FFmpeg directory, it's probably
            # because when it was extracted the useful stuff was left inside a
            # nested directory. If we detect this we can pull all of that out
            # into the root `ffmpeg` directory (but we still ask first).
            if len(ffmpeg_dir_list := os.listdir(ffmpeg_dir)) == 1 and confirm(
                "FFmpeg directory contains only 1 subfolder"
                + f" `{ffmpeg_dir_list[0]}`."
                + " Attempt auto-fix?"
            ):
                try:
                    nested_dir = f"{ffmpeg_dir}\\{ffmpeg_dir_list[0]}"
                    for nested_child in os.listdir(nested_dir):
                        shutil.move(
                            f"{nested_dir}\\{nested_child}",
                            f"{ffmpeg_dir}\\{nested_child}",
                        )
                    os.rmdir(nested_dir)
                except:
                    warning("FFmpeg directory structure fix failed.")
                    if not confirm("Auto-fix failed. Continue anyway?"):
                        fatal("Not continuing.")
                else:
                    info("FFmpeg directory structure fix attempted.")

            ensure_path_exists(f"{ffmpeg_dir}\\include")
            ensure_path_exists(f"{ffmpeg_dir}\\lib")
            ensure_path_exists(f"{ffmpeg_dir}\\bin")
            info("FFmpeg found locally.")

            if os.path.exists(FFMPEG_ZIP_PATH) and confirm(
                "Auto-downloaded FFmpeg zip file no longer needed."
                + f" Would you like to remove it (`{FFMPEG_ZIP_PATH}`)?",
            ):
                os.remove(FFMPEG_ZIP_PATH)

            return ffmpeg_dir

        # We can at least ask to download the FFmpeg zip file automatically if
        # they don't have it.

        FFMPEG_DOWNLOAD_URL = "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.0.1-full_build-shared.7z"

        MANUAL_INSTALL_MSG = (
            "You can still manually install.\n"
            + "Please rerun this script after downloading and extracting"
            + f" FFmpeg to `{FFMPEG_DIR_LOCAL}`"
            + f" from the link below (or anywhere):\n{FFMPEG_DOWNLOAD_URL}"
        )

        if not confirm(
            "You don't have FFmpeg installed locally yet."
            + " Do you want to download FFmpeg from the internet now?"
        ):
            fatal(f"Skipping auto-download. {MANUAL_INSTALL_MSG}")

        info(
            "Installing FFmpeg. This may take a while... ",
            end="",
            flush=True,
        )

        try:
            urllib.request.urlretrieve(FFMPEG_DOWNLOAD_URL, FFMPEG_ZIP_PATH)
        except Exception as e:
            if isinstance(e, KeyboardInterrupt):
                try:
                    os.remove(FFMPEG_ZIP_PATH)
                except:
                    pass
                print("")
                warning(f"Download cancelled. {MANUAL_INSTALL_MSG}")
                raise
            fatal(f"\nDownload failed. {MANUAL_INSTALL_MSG}")
        print("Done.")
        info("FFmpeg zip file downloaded.")

        # Start from the top.
        return get_ffmpeg_dir()

    libclang_path = get_libclang_path()
    clang_include_dir = try_to_get_clang_include_dir()
    ffmpeg_dir = get_ffmpeg_dir()

    def to_unix_path(path: str) -> str:
        return path.replace("\\", "/")

    # We need to set `LIBCLANG_PATH` so that `ffmpeg-next` can build its
    # bindings.
    # We also need to set `FFMPEG_DIR` so that `ffmpeg-next` has FFmpeg's actual
    # lib and include files. This is required because you can only dynamically
    # link with FFmpeg.
    # See https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building
    cargo_config = (
        "[env]\n"
        + f'LIBCLANG_PATH = "{to_unix_path(libclang_path)}"\n'
        + f'FFMPEG_DIR = "{to_unix_path(ffmpeg_dir)}"\n'
    )
    if clang_include_dir is not None:
        # If we found Clang's include directory we'll explicitly pass it to
        # `bindgen` (the library `ffmpeg-next` uses to generate rust bindings)
        # so that it doesn't get confused and try to build using mingw headers
        # or something else weird.
        cargo_config += (
            "BINDGEN_EXTRA_CLANG_ARGS = "
            + f'"-I{to_unix_path(clang_include_dir)}"\n'
        )

    create_cargo_config(cargo_config)

    run_cmd("cargo", "clean")
    info("Build directory cleaned.")


# Handles build setup for MacOS builds.
def mac_os() -> None:
    arch = get_supported_arch()
    if arch is None:
        fatal("MacOS builds only support x86_64 and arm64")

    def ensure_brew_is_installed() -> None:
        try:
            ensure_cmd_exists("brew", non_fatal=True)
        except DoesntExistException:
            if not confirm(
                "You do not have the Homebrew package manager installed."
                + " Start Homebrew install? (you'll need to be a system admin)"
            ):
                fatal("Homebrew is required.")

            # This comes from the download instructions here: https://brew.sh/
            run_cmd(
                '/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"',
                shell=True,
            )
            ensure_cmd_exists("brew")

        info("Found Homebrew package manager.")

    def is_installed_with_brew(
        pkg_name: str, ask_to_install: bool = False
    ) -> bool:
        installed_pkgs = typing.cast(
            list[dict[str, Any]],
            json.loads(run_cmd("brew", "info", "--installed", "--json")),
        )

        def names_from_pkg_dict(pkg_dict: dict[str, Any]) -> list[str]:
            ret: list[str] = []
            for key in ("name", "full_name", "oldnames", "aliases"):
                val = pkg_dict.get(key)
                if isinstance(val, str):
                    ret.append(val)
                elif isinstance(val, list):
                    ret.extend(
                        sub_val
                        for sub_val in typing.cast(list[Union[str, Any]], val)
                        if isinstance(sub_val, str)
                    )
            return ret

        is_already_installed = any(
            pkg_name in names_from_pkg_dict(pkg_dict)
            for pkg_dict in installed_pkgs
        )

        if is_already_installed or not ask_to_install:
            return is_already_installed

        if not confirm(
            f"It doesn't look like you have `{pkg_name}` installed."
            + " Install with Homebrew?"
        ):
            return False

        info(
            f"Installing `{pkg_name}` with Homebrew."
            + " This can take a very long time."
        )
        run_cmd("brew", "install", pkg_name)
        return True

    ensure_brew_is_installed()
    if not is_installed_with_brew("ffmpeg@8", ask_to_install=True):
        warning("Continuing without installing FFmpeg (8).")
    if not is_installed_with_brew("pkg-config", ask_to_install=True):
        warning("Continuing without installing `pkg-config`.")

    run_cmd("cargo", "clean")
    info("Build directory cleaned.")


def main() -> None:
    try:
        parse_args()

        ensure_cmd_exists("cargo")

        system = platform.system().lower()
        if system == "windows":
            windows()
        elif system == "darwin":  # MacOS
            mac_os()
        elif system == "linux":
            fatal("unimplemented")
        else:
            fatal(f"Unsupported system: `{system}`")

        success("Build setup complete. Try running `cargo build`.")

    except KeyboardInterrupt:
        print(Color.RESET, end="")
        print(Color.RESET, file=sys.stderr)
        fatal(f"Stop signal received.", include_run_again_msg=False)


if __name__ == "__main__":
    main()
