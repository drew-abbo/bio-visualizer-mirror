"""
Contains logging functions.
"""

import sys
import os
from typing import NoReturn, Any, Optional


class Color:
    __output_is_terminal = sys.stdout.isatty()

    ERROR = "\033[31m" if __output_is_terminal else ""
    WARNING = "\033[33m" if __output_is_terminal else ""
    INFO = "\033[36m" if __output_is_terminal else ""
    SUCCESS = "\033[32m" if __output_is_terminal else ""
    CONFIRM = "\033[35m" if __output_is_terminal else ""
    ACTION_NEEDED = "\033[35m\033[1m" if __output_is_terminal else ""
    COMMAND = "\033[34m" if __output_is_terminal else ""
    RESET = "\033[0m" if __output_is_terminal else ""


# Print an error and exit.
def fatal(
    *args: Any,
    include_run_again_msg: bool = True,
    sep: Optional[str] = " ",
) -> NoReturn:
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


# Print a warning.
def warning(
    *args: Any,
    sep: Optional[str] = " ",
    end: Optional[str] = "\n",
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
    sep: Optional[str] = " ",
    end: Optional[str] = "\n",
    flush: bool = False,
) -> None:
    print(f"{Color.INFO}INFO{Color.RESET}: ", end="", flush=False)
    print(*args, sep=sep, end=end, flush=flush)


# Print that the process is done (success).
def success(
    *args: Any,
    sep: Optional[str] = " ",
    end: Optional[str] = "\n",
    flush: bool = False,
) -> None:
    print(f"{Color.SUCCESS}SUCCESS{Color.RESET}: ", end="", flush=False)
    print(*args, sep=sep, end=end, flush=flush)
