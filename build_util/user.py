"""
Contains utilities for getting user input.
"""

from typing import Any, Optional

from . import sh
from . import log
from .log import Color

__confirm_auto_answer = None


def set_confirm_auto_answer(auto_answer: Optional[str]) -> None:
    """
    When set to a value other than `None`, `confirm()` will be replied to
    automatically with this value instead of prompting the user.
    """

    global __confirm_auto_answer
    __confirm_auto_answer = auto_answer


def confirm(*args: Any, sep: Optional[str] = " ") -> bool:
    """
    Log a message and await a "yes"/"no" from the user.
    """

    print(f"{Color.CONFIRM}CONFIRM{Color.RESET}: ", end="")
    print(*args, sep=sep, end="")
    print(f" ({Color.CONFIRM}y{Color.RESET}/n): {Color.CONFIRM}", end="")

    if __confirm_auto_answer is None:
        try:
            response = input().strip().lower()
        except KeyboardInterrupt:
            print(f"{Color.RESET}({Color.ERROR}cancelled{Color.RESET})")
            response = ""

        print(f"{Color.RESET}", end="", flush=True)
    else:
        response = __confirm_auto_answer
        print(f"{__confirm_auto_answer}{Color.RESET} (auto)")

    return response in ("y", "yes")


def action_needed(*args: Any, sep: Optional[str] = " ") -> None:
    """
    Log a message and wait for the user to hit enter, allowing them to
    optionally run a command.
    """

    print(f"{Color.ACTION_NEEDED}MANUAL ACTION NEEDED{Color.RESET}: ", end="")
    print(*args, sep=sep, end="")
    print(
        f" (press [{Color.ACTION_NEEDED}ENTER{Color.RESET}] if you have "
        + "completed the action manually or enter a shell command to run): "
        + f"{Color.ACTION_NEEDED}",
        end="",
    )

    output_is_terminal = Color.RESET != ""

    try:
        response = input()
    except KeyboardInterrupt:
        print(f"{Color.RESET}({Color.ERROR}cancelled{Color.RESET})")
        log.fatal("No input supplied.", include_run_again_msg=False)

    if not output_is_terminal:
        print(f"{response}{Color.RESET} (auto)")
    print(f"{Color.RESET}", end="", flush=True)

    if len(response) != 0 and not response.isspace():
        sh.run_cmd(response, shell=True)
