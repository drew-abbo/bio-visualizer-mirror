"""
Contains logging functions.
"""

import sys
import os
from typing import NoReturn, Any, Optional


class Color:
    ENABLED = bool(sys.stdout.isatty())

    ERROR = "\033[31m" if ENABLED else ""
    WARNING = "\033[33m" if ENABLED else ""
    INFO = "\033[36m" if ENABLED else ""
    SUCCESS = "\033[32m" if ENABLED else ""
    CONFIRM = "\033[35m" if ENABLED else ""
    ACTION_NEEDED = "\033[35m\033[1m" if ENABLED else ""
    COMMAND = "\033[34m" if ENABLED else ""
    RESET = "\033[0m" if ENABLED else ""


def fatal(
    *args: Any,
    include_run_again_msg: bool = True,
    sep: Optional[str] = " ",
) -> NoReturn:
    """
    Print an error and exit.
    """

    sys.stdout.flush()

    print(f"{Color.ERROR}FATAL{Color.RESET}: ", end="", file=sys.stderr)
    print(*args, sep=sep, file=sys.stderr)
    if include_run_again_msg:
        print(
            "\nPlease run this script again once the issue is resolved.",
            file=sys.stderr,
            flush=True,
        )

    os._exit(1)


def warning(
    *args: Any,
    sep: Optional[str] = " ",
    end: Optional[str] = "\n",
    flush: bool = False,
) -> None:
    """
    Print a warning.
    """

    print(
        f"{Color.WARNING}WARNING{Color.RESET}: ",
        end="",
        file=sys.stderr,
        flush=False,
    )
    print(*args, sep=sep, file=sys.stderr, end=end, flush=flush)


def error(
    *args: Any,
    sep: Optional[str] = " ",
    end: Optional[str] = "\n",
    flush: bool = False,
) -> None:
    """
    Print an error.
    """

    print(
        f"{Color.ERROR}ERROR{Color.RESET}: ",
        end="",
        file=sys.stderr,
        flush=False,
    )
    print(*args, sep=sep, file=sys.stderr, end=end, flush=flush)


def info(
    *args: Any,
    sep: Optional[str] = " ",
    end: Optional[str] = "\n",
    flush: bool = False,
) -> None:
    """
    Print some info.
    """

    print(f"{Color.INFO}INFO{Color.RESET}: ", end="", flush=False)
    print(*args, sep=sep, end=end, flush=flush)


def success(
    *args: Any,
    sep: Optional[str] = " ",
    end: Optional[str] = "\n",
    flush: bool = False,
) -> None:
    """
    Print that the process is done (success).
    """

    print(f"{Color.SUCCESS}SUCCESS{Color.RESET}: ", end="", flush=False)
    print(*args, sep=sep, end=end, flush=flush)
