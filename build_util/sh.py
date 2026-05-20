"""
Contains shell and OS utilities.
"""

import os
import platform
import sys
import shutil
import subprocess
import typing
from pathlib import Path
from typing import Literal, Iterable, Sequence, Optional, Callable

from . import log
from .log import Color


def get_supported_arch() -> Optional[Literal["x86_64", "arm64"]]:
    """
    Returns `"x86_64"`, `"arm64"`, or `None` depending on the architecture of
    the current machine.
    """

    return typing.cast(
        Optional[Literal["x86_64", "arm64"]],
        {
            "x86_64": "x86_64",
            "amd64": "x86_64",
            "arm64": "arm64",
        }.get(platform.machine().lower()),
    )


def rm_path(
    path: str,
    allow_missing: bool = False,
    help_msg: Optional[str] = None,
    non_fatal: bool = False,
) -> bool:
    """
    Removes a file or directory (and its contents). Whether the file existed or
    not is returned.
    """

    if not os.path.exists(path):
        if allow_missing:
            return False

        err_msg = f"Couldn't remove `{path}` because it doesn't exist." + (
            f"\n{help_msg}" if help_msg is not None else ""
        )
        if non_fatal:
            raise DoesntExistException(err_msg)
        log.fatal(err_msg)

    try:
        if os.path.isdir(path):
            shutil.rmtree(path)
        elif os.path.exists(path):
            os.remove(path)
    except:
        err_msg = f"Failed to remove `{path}`." + (
            f"\n{help_msg}" if help_msg is not None else ""
        )
        if non_fatal:
            raise RemovePathException(err_msg)
        log.fatal(err_msg)

    return True


def ensure_path_exists(
    path: str,
    kind: Literal["file", "dir", "any"] = "any",
    help_msg: Optional[str] = None,
    non_fatal: bool = False,
) -> None:
    """
    Does a check to see if a path exists.
    """

    if kind == "file":
        check_exists = os.path.isfile
        kind_str = "file"
    elif kind == "dir":
        check_exists = os.path.isdir
        kind_str = "directory"
    elif kind == "any":
        check_exists = os.path.exists
        kind_str = "anything"

    if check_exists(path):
        return

    err_msg = f"Couldn't find {kind_str} at `{path}`." + (
        f"\n{help_msg}" if help_msg is not None else ""
    )
    if non_fatal:
        raise DoesntExistException(err_msg)
    log.fatal(err_msg)


def copy_files_dir_to_dir(
    src_dir: str, dest_dir: str, file_ext_filter: Optional[str] = None
) -> list[str]:
    """
    Copy regular files from `src_dir` (non-recursive) to `dest_dir`. If
    `file_ext_filter` isn't `None`, only files with a matching file extension
    will be copied. A list of copied destination file paths is returned.
    """

    ensure_path_exists(src_dir, kind="dir")

    if file_ext_filter is not None and not file_ext_filter.startswith("."):
        file_ext_filter = f".{file_ext_filter}"

    file_kind = "all" if file_ext_filter is None else f"`{file_ext_filter}`"
    log.info(f"Copying {file_kind} files from `{src_dir}` to `{dest_dir}`.")

    copied: list[str] = []
    try:
        for file_name in os.listdir(src_dir):
            file_path = f"{src_dir}/{file_name}"

            if not os.path.isfile(file_path) or (
                file_ext_filter is not None
                and not file_name.endswith(file_ext_filter)
            ):
                continue

            dest_path = f"{dest_dir}/{file_name}"
            shutil.copy(file_path, dest_path)
            copied.append(os.path.abspath(dest_path))
    except:
        log.fatal("Failed top copy files from one directory to another.")
    return copied


def ensure_cmd_exists(
    cmd: str, help_msg: Optional[str] = None, non_fatal: bool = False
) -> None:
    """
    Does a check to see if a command exists on the `PATH` or the file system.
    """

    if shutil.which(cmd) is not None or os.path.exists(cmd):
        return

    err_msg = f"Couldn't find command `{cmd}`." + (
        f"\n{help_msg}" if help_msg is not None else ""
    )
    if non_fatal:
        raise DoesntExistException(err_msg)
    log.fatal(err_msg)


def run_cmd(
    *cmd: str,
    shell: bool = False,
    non_fatal: bool = False,
    show_output: bool = True,
    env_overrides: Optional[dict[str, str]] = None,
) -> str:
    """
    Runs a shell command and returns its output (minus a trailing newline if it
    has one). All CRLF newlines are replaced with LF newlines in the returned
    output.
    """

    __print_running_cmd(cmd, env_overrides)

    if show_output:
        print(f"{Color.COMMAND}{' OUTPUT ':~^80}{Color.RESET}", flush=True)

    try:
        if env_overrides is None:
            env_overrides = {}
        env = os.environ.copy()
        for k, v in env_overrides.items():
            env[k] = v

        process = subprocess.Popen(
            cmd if not shell else " ".join(cmd),
            shell=shell,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,  # Combine stderr and stdout to stdout.
            text=False,
            env=env,
        )

        # Capture lines as they come in.
        output = ""
        if process.stdout is not None:
            for line in process.stdout:
                line = line.decode("utf-8", errors="replace")
                output += line
                if show_output:
                    print(line, end="", flush=True)
        process.wait()

    except KeyboardInterrupt:
        raise
    except Exception as e:
        raise CmdException(
            f"Couldn't run `{__format_cmd(cmd, env_overrides)}`: {e}."
        )
    finally:
        if show_output:
            print(f"\n{Color.RESET + Color.COMMAND}{'~' * 80}{Color.RESET}")

    if (exit_code := process.returncode) != 0:
        err_msg = (
            f"`{__format_cmd(cmd, env_overrides)}` "
            + f"failed with exit code {exit_code}."
        )
        if non_fatal:
            raise CmdException(err_msg)
        log.fatal(err_msg)

    output = output.replace("\r\n", "\n")
    if output.endswith("\n"):
        output = output[:-1]
    return output


def start_cmd(*cmd: str, shell: bool = False) -> None:
    """
    Like `run_cmd()` except it doesn't wait for the command to finish.
    """

    __print_running_cmd(cmd, None)
    subprocess.Popen(cmd if not shell else " ".join(cmd), shell=shell)


def require_script_in_working_dir() -> None:
    """
    Exits if the caller isn't running the script from the same directory as the
    script.
    """

    try:
        script_dir = str(Path(os.path.realpath(sys.argv[0])).parent)
        working_dir = os.path.realpath(os.getcwd())
    except:
        log.fatal("Failed to compare script directory with working directory.")

    if script_dir != working_dir:
        log.fatal(
            "This script cannot be run from another directory. "
            + f"Run from `{script_dir}`."
        )


class CmdException(Exception):
    """
    Raised if something goes wrong running a command.
    """


class DoesntExistException(Exception):
    """
    Raised if something doesn't exist.
    """


class RemovePathException(Exception):
    """
    Raised if something failed to be removed.
    """


def catch_stop_signal(fn: Callable[[], None]) -> None:
    try:
        fn()
    except KeyboardInterrupt:
        print(Color.RESET, end="")
        print(Color.RESET, file=sys.stderr)
        log.fatal(f"Stop signal received.", include_run_again_msg=False)


def __format_cmd(
    cmd: Iterable[str],
    env_overrides: Optional[dict[str, str]],
) -> str:
    """
    Joins the command arguments into a single string, naively wrapping arguments
    that contain spaces in double quotes.
    """

    def clean_arg(arg: str) -> str:
        return f'"{arg}"' if " " in arg else arg

    joined_cmd = " ".join(clean_arg(arg) for arg in cmd)

    if env_overrides is not None and len(env_overrides) != 0:
        joined_cmd = (
            " ".join(f"{k}={clean_arg(v)}" for k, v in env_overrides.items())
            + f" {joined_cmd}"
        )

    return joined_cmd


def __print_running_cmd(
    cmd: Sequence[str],
    env_overrides: Optional[dict[str, str]],
) -> None:
    """
    Highlight the file name in the first argument.
    """

    last_slash_idx = max(cmd[0].rfind("/"), cmd[0].rfind("\\"))
    highlight_start_idx = 0 if last_slash_idx == -1 else last_slash_idx + 1

    cmd = [
        f"{cmd[0][:highlight_start_idx]}"
        + f"{Color.COMMAND}{cmd[0][highlight_start_idx:]}{Color.RESET}",
        *cmd[1:],
    ]

    print(
        f"{Color.COMMAND}RUNNING COMMAND{Color.RESET}: "
        + f"`{__format_cmd(cmd, env_overrides)}`"
    )
