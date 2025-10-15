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
def fatal(*args, include_run_again_msg: bool = True, **kwargs) -> None:
    print(f"{Color.ERROR}FATAL{Color.RESET}: ", end="", file=sys.stderr)
    print(*args, **kwargs, file=sys.stderr)
    if include_run_again_msg:
        print(
            "\nPlease run this script again once the issue is resolved.",
            file=sys.stderr,
        )
    sys.exit(1)


# Print a warning.
def warning(*args, **kwargs) -> None:
    print(f"{Color.WARNING}WARNING{Color.RESET}: ", end="", file=sys.stderr)
    print(*args, **kwargs, file=sys.stderr)


# Print some info.
def info(*args, **kwargs) -> None:
    print(f"{Color.INFO}INFO{Color.RESET}: ", end="")
    print(*args, **kwargs)


# Print that the process is done (success).
def success(*args, **kwargs) -> None:
    print(f"{Color.SUCCESS}SUCCESS{Color.RESET}: ", end="")
    print(*args, **kwargs)


# When set, all `confirm` confirmations with be responded to with this instead
# of prompting the user.
confirm_auto_answer = None


# Print a message and await a "yes"/"no" from the user.
def confirm(*args, **kwargs) -> bool:
    print(f"{Color.CONFIRM}CONFIRM{Color.RESET}: ", end="")
    print(*args, **kwargs, end="")
    print(f" ({Color.CONFIRM}y{Color.RESET}/n): {Color.CONFIRM}", end="")

    if confirm_auto_answer is None:
        response = input().strip().lower()
        print(f"{Color.RESET}", end="", flush=True)
    else:
        response = confirm_auto_answer
        print(f"{confirm_auto_answer}{Color.RESET} (auto)")

    return response in ("y", "yes")


# Print a message and wait for the user to hit enter.
def action_needed(*args, **kwargs) -> bool:
    print(f"{Color.ACTION_NEEDED}MANUAL ACTION NEEDED{Color.RESET}: ", end="")
    print(*args, **kwargs, end="")
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

    if not response.isspace():
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


# Does a check to see if a path exists.
def ensure_path_exists(
    path: str, help_msg: str | None = None, non_fatal: bool = False
):
    if not os.path.exists(path):
        err_msg = f"Couldn't find `{path}`." + (
            f"\n{help_msg}" if help_msg is not None else ""
        )
        if non_fatal:
            raise DoesntExistException(err_msg)
        fatal(err_msg)


# Does a check to see if a command exists on the `PATH`.
def ensure_cmd_exists(
    cmd: str, help_msg: str | None = None, non_fatal: bool = False
):
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
def format_cmd(cmd: list[str]) -> str:
    return " ".join(arg if " " not in arg else f'"{arg}"' for arg in cmd)


def print_running_cmd(cmd: list[str]) -> None:
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
        for line in process.stdout:
            output += line
            print(line, end="", flush=True)
        process.wait()

    except KeyboardInterrupt:
        raise
    finally:
        print(f"\n{Color.COMMAND}{'~' * 80}{Color.RESET}")

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

    info("Cargo config generated.")


# Handles build setup for Windows builds.
def windows() -> None:
    if platform.machine().lower() not in ("amd64", "x86_64"):
        fatal("Windows builds currently only support x86_64.")

    vs_installer_dir = (
        os.environ.get("ProgramFiles(x86)")
        + "\\Microsoft Visual Studio\\Installer"
    )

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
            "[17.0, 18.0)",  # Only Visual Studio 2022.
            "-latest",
            non_fatal=True,
        )
    except CmdException:
        fatal("Couldn't find Visual Studio 2022.")
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
    def try_to_get_clang_include_dir() -> str | None:
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
                nested_dir = f"{ffmpeg_dir}\\{ffmpeg_dir_list[0]}"
                for nested_child in os.listdir(nested_dir):
                    shutil.move(
                        f"{nested_dir}\\{nested_child}",
                        f"{ffmpeg_dir}\\{nested_child}",
                    )
                os.rmdir(nested_dir)
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

        FFMPEG_DOWNLOAD_URL = (
            "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full-shared.7z"
        )

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

    # We need to set `LIBCLANG_PATH` so that `ffmpeg-next` can build its
    # bindings.
    # We also need to set `FFMPEG_DIR` so that `ffmpeg-next` has FFmpeg's actual
    # lib and include files. This is required because you can only dynamically
    # link with FFmpeg.
    # See https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building
    cargo_config = (
        "[env]\n"
        + f'LIBCLANG_PATH = "{libclang_path.replace("\\", "/")}"\n'
        + f'FFMPEG_DIR = "{ffmpeg_dir.replace("\\", "/")}"\n'
    )
    if clang_include_dir is not None:
        # If we found Clang's include directory we'll explicitly pass it to
        # `bindgen` (the library `ffmpeg-next` uses to generate rust bindings)
        # so that it doesn't get confused and try to build using mingw headers
        # or something else weird.
        cargo_config += (
            "BINDGEN_EXTRA_CLANG_ARGS = "
            + f'"-I{clang_include_dir.replace("\\", "/")}"\n'
        )

    create_cargo_config(cargo_config)

    ensure_cmd_exists("cargo")
    run_cmd("cargo", "clean")
    info("Build directory cleaned.")

    success("Build setup complete. Try running `cargo build`.")


def main() -> None:
    try:
        parse_args()

        system = platform.system()
        if system == "Windows":
            windows()
        elif system == "Darwin":  # MacOS
            fatal("unimplemented")
        elif system == "Linux":
            fatal("unimplemented")
        else:
            fatal(f"Unsupported system: `{system}`")

    except KeyboardInterrupt:
        print(Color.RESET, end="")
        print(Color.RESET, file=sys.stderr)
        fatal(f"Stop signal received.", include_run_again_msg=False)


if __name__ == "__main__":
    main()
