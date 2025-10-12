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

import os
import platform
import shutil
import subprocess
import sys
import urllib.request


class Color:
    ERROR = "\033[31m"
    WARNING = "\033[33m"
    INFO = "\033[36m"
    SUCCESS = "\033[32m"
    CONFIRM = "\033[35m"
    ACTION_NEEDED = "\033[35m\033[1m"
    RESET = "\033[0m"


# Print an error and exit.
def fatal(*args, **kwargs):
    print(f"{Color.ERROR}FATAL{Color.RESET}: ", end="", file=sys.stderr)
    print(*args, **kwargs, file=sys.stderr)
    print("\nPlease run this script again once the issue is resolved.")
    sys.exit(1)


# Print a warning.
def warning(*args, **kwargs):
    print(f"{Color.WARNING}WARNING{Color.RESET}: ", end="", file=sys.stderr)
    print(*args, **kwargs, file=sys.stderr)


# Print some info.
def info(*args, **kwargs):
    print(f"{Color.INFO}INFO{Color.RESET}: ", end="")
    print(*args, **kwargs)


# Print some info.
def success(*args, **kwargs):
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

    response = input().strip()
    print(f"{Color.RESET}", end="", flush=True)

    if len(response) != 0:
        run_cmd(response, shell=True)


# Parses command line arguments.
def parse_args():
    arg0 = sys.argv[0]
    for arg in sys.argv[1:]:
        if arg == "-y":
            global confirm_auto_answer
            confirm_auto_answer = "y"
        else:
            fatal(f"Unknown argument `{arg}`.\n" + f"Usage: {arg0} [-y]")


# Does a check to see if a path exists.
def ensure_path_exists(path: str, help_msg=None, non_fatal=False):
    if not os.path.exists(path):
        err_msg = f"Couldn't find `{path}`." + (
            f"\n{help_msg}" if help_msg is not None else ""
        )
        if non_fatal:
            raise Exception(err_msg)
        else:
            fatal(err_msg)


# Does a check to see if a command exists on the `PATH`.
def ensure_cmd_exists(cmd: str, help_msg=None, non_fatal=False):
    if not shutil.which(cmd):
        err_msg = f"Couldn't find `{cmd}` on the path." + (
            f"\n{help_msg}" if help_msg is not None else ""
        )
        if non_fatal:
            raise Exception(err_msg)
        else:
            fatal(err_msg)


# Runs a shell command and returns its `stdout` output (minus a trailing newline
# if it has one).
def run_cmd(
    *cmd: str, keep_trailing_newline=False, shell=False, non_fatal=False
) -> str:
    try:
        ret = subprocess.run(
            cmd if not shell else " ".join(cmd),
            shell=shell,
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    except:
        err_msg = f"`{" ".join(cmd)}` failed."
        if non_fatal:
            raise Exception(err_msg)
        else:
            fatal(err_msg)

    if keep_trailing_newline or len(ret) == 0 or ret[-1] != "\n":
        return ret
    return ret[:-1]


# Create a `.cargo/config.toml` file.
def create_cargo_config(contents: str):
    if os.path.exists("./.cargo/config.toml") and not confirm(
        "A `.cargo/config.toml` file already exists. Overwrite?"
    ):
        fatal("Failed to create `.cargo/config.toml`.")
    else:
        os.makedirs("./.cargo", exist_ok=True)
    with open("./.cargo/config.toml", "w", encoding="utf-8") as f:
        f.write("# Generated by `build_setup.py`.\n" + contents)

    info("Cargo config generated.")


# Handles build setup for Windows builds.
def windows():
    if platform.machine().lower() not in ("amd64", "x86_64"):
        fatal("Windows builds currently only support x86_64.")

    def get_msvc_install_dir() -> str:
        # `vswhere` is a utility that MSVC provides for finding where MSVC
        # installed everything.
        vswhere = (
            os.environ.get("ProgramFiles(x86)").replace("\\", "/")
            + "/Microsoft Visual Studio/Installer/vswhere.exe"
        )
        ensure_path_exists(
            vswhere,
            help_msg="You likely don't have `Visual Studio Installer`"
            + " on your system. Please install it from here:\n"
            + "https://visualstudio.microsoft.com/",
        )

        msvc_install_dir = run_cmd(
            vswhere, "-latest", "-property", "installationPath"
        ).replace("\\", "/")
        info("MSVC found.")

        return msvc_install_dir

    # Make sure we have the C-compiler dependencies needed to generate rust
    # bindings for FFmpeg.
    def get_libclang_path(msvc_install_dir: str) -> str:
        # `libclang` needs to be installed for FFmpeg-next to be able to create
        # rust bindings.
        libclang_path = f"{msvc_install_dir}/VC/Tools/LLVM/x64/bin"
        ensure_path_exists(
            f"{libclang_path}/libclang.dll",
            help_msg="You likely don't have libclang installed.\n"
            + "Please use `Visual Studio Installer` to add the "
            + "`C++ Clang Compiler for Windows` component to your system.",
        )
        info("libclang found.")

        return libclang_path

    # Try and ensure we're using the right header files for generating rust
    # bindings for FFmpeg.
    def get_clang_include_dir(msvc_install_dir: str) -> str:
        # If you have some other C compiler's header files (e.g. mingw) then the
        # build can fail because it might use the wrong compiler's header files.
        # If we can find clang's header files then we can directly tell it to
        # prioritize those over anything else it may find.
        try:
            clang_dir = f"{msvc_install_dir}/VC/Tools/LLVM/x64/lib/clang"
            ensure_path_exists(clang_dir, non_fatal=True)
            newest_clang_version = sorted(
                version
                for version in os.listdir(clang_dir)
                if os.path.isdir(os.path.join(clang_dir, version))
            )[-1]
            clang_include_dir = f"{clang_dir}/{newest_clang_version}/include"
            info("clang's include directory found.")
        except:
            warning("Failed to find clang's include directory.")
            clang_include_dir = None

        return clang_include_dir

    # Make sure we have FFmpeg installed in the project directory.
    def get_ffmpeg_dir() -> str:
        FFMPEG_ZIP_PATH = "./ffmpeg.7z"
        FFMPEG_DIR_LOCAL = "./ffmpeg"
        ffmpeg_dir = os.path.abspath(FFMPEG_DIR_LOCAL).replace("\\", "/")

        ffmpeg_dir_exists = os.path.exists(ffmpeg_dir)

        # If they've already got a downloaded zip file for it then we can just
        # tell them to extract it. We can't extract it for them because the
        # download is a 7z file for some reason and there's no way to extract a
        # 7z file on windows without using the file explorer UI or downloading
        # an external utility. The external utility we could download (7za)
        # comes as a zipped .7z file so we can't even do that automatically.
        if os.path.exists(FFMPEG_ZIP_PATH) and not ffmpeg_dir_exists:
            info("Attempting to open file explorer on FFmpeg zip file.")
            try:
                run_cmd(
                    "explorer",
                    "/select,",
                    f"{os.path.abspath(FFMPEG_ZIP_PATH)}",
                    non_fatal=True,
                )
            except:
                pass
            action_needed(
                f"Please extract `{FFMPEG_ZIP_PATH}`" + f" to `{FFMPEG_DIR_LOCAL}`.",
            )

            ffmpeg_dir_exists = os.path.exists(ffmpeg_dir)
            if not ffmpeg_dir_exists:
                fatal("The FFmpeg directory wasn't extracted.")

        if ffmpeg_dir_exists:
            # If there's only 1 item in the FFmpeg directory, it's probably
            # because when it was extracted the useful stuff was left inside a
            # nested directory. If we detect this we can pull all of that out
            # into the root `ffmpeg` directory (but we still ask first).
            if len(ffmpeg_dir_list := os.listdir(ffmpeg_dir)) == 1:
                if confirm(
                    "FFmpeg directory contains only 1 subfolder"
                    + f" `{ffmpeg_dir_list[0]}`."
                    + " Attempt auto-fix?"
                ):
                    nested_dir = f"{ffmpeg_dir}/{ffmpeg_dir_list[0]}"
                    for nested_child in os.listdir(nested_dir):
                        shutil.move(
                            f"{nested_dir}/{nested_child}",
                            f"{ffmpeg_dir}/{nested_child}",
                        )
                    os.rmdir(nested_dir)
                    info("FFmpeg directory structure fix attempted.")

            ensure_path_exists(f"{ffmpeg_dir}/include")
            ensure_path_exists(f"{ffmpeg_dir}/lib")
            ensure_path_exists(f"{ffmpeg_dir}/bin")
            info("FFmpeg found locally.")

            if os.path.exists(FFMPEG_ZIP_PATH):
                if confirm(
                    "Auto-downloaded FFmpeg zip file no longer needed."
                    + f" Would you like to remove it (`{FFMPEG_ZIP_PATH}`)?",
                ):
                    os.remove(FFMPEG_ZIP_PATH)

            return ffmpeg_dir

        # We can at least ask to download the FFmpeg zip file automatically.

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
        except:
            fatal(f"\nDownload failed. {MANUAL_INSTALL_MSG}")
        print("Done.")
        info("FFmpeg zip file downloaded.")

        return get_ffmpeg_dir()

    msvc_install_dir = get_msvc_install_dir()
    libclang_path = get_libclang_path(msvc_install_dir)
    clang_include_dir = get_clang_include_dir(msvc_install_dir)
    ffmpeg_dir = get_ffmpeg_dir()

    # See https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building
    # We need to set `LIBCLANG_PATH` so that ffmpeg-next can build its bindings.
    # We need to set `FFMPEG_DIR` so that ffmpeg-next has FFmpeg's actual lib
    # and include files. This is requires because you can't build with FFmpeg
    # statically. You have to dynamically link it.
    cargo_config = (
        "[env]\n"
        + f'LIBCLANG_PATH = "{libclang_path}"\n'
        + f'FFMPEG_DIR = "{ffmpeg_dir}"\n'
    )
    if clang_include_dir is not None:
        # If we found clang's include directory we'll explicitly pass it to
        # bindgen so that it doesn't get confused and try to build using mingw
        # headers or something else weird.
        cargo_config += f'BINDGEN_EXTRA_CLANG_ARGS = "-I{clang_include_dir}"\n'

    create_cargo_config(cargo_config)

    ensure_cmd_exists("cargo")
    run_cmd("cargo", "clean")
    info("Build directory cleaned.")

    success("Build setup complete. Try running `cargo build`.")


def main():
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


if __name__ == "__main__":
    main()
