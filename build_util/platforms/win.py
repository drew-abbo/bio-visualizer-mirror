"""
Contains Windows-specific utilities.
"""

import os
import platform
from functools import cache

from .. import log
from .. import sh

assert platform.system().lower() == "windows"


@cache
def program_files(x86: bool = False) -> str:
    """
    The path to the program files. The result is cached.
    """

    path = os.environ.get("ProgramFiles(x86)" if x86 else "ProgramFiles")
    if path is None:
        log.fatal("Coldn't find program files.")
    return path


@cache
def vs_installer_dir() -> str:
    """
    The path that the Visual Studio Installer is in. The result is cached.
    """

    path_suffix = "\\Microsoft Visual Studio\\Installer"

    # We'll check both program files folders.
    try:
        ret = program_files(x86=True) + path_suffix
        sh.ensure_path_exists(ret, kind="dir", non_fatal=True)
    except sh.DoesntExistException:
        ret = program_files() + path_suffix
        sh.ensure_path_exists(
            ret,
            kind="dir",
            help_msg="You likely don't have the Visual Studio Installer"
            + " on your system. Please install it from here:\n"
            + "https://visualstudio.microsoft.com/",
        )

    log.info("Found the Visual Studio's Installer.")
    return ret


@cache
def vs_installation_dir() -> str:
    """
    The path of a Visual Studio installation. The result is cached.
    """

    sh.ensure_path_exists(f"{vs_installer_dir()}\\vswhere.exe", kind="file")
    try:
        ret = sh.run_cmd(
            f"{vs_installer_dir()}\\vswhere.exe",
            "-property",
            "installationPath",
            "-version",
            "[17.0,19.0)",  # Only Visual Studio 2022 or 2026.
            "-latest",
            non_fatal=True,
        )
    except sh.CmdException:
        log.fatal("Couldn't find Visual Studio.")

    log.info("Visual Studio found.")
    return ret


@cache
def vs_msvc_tools_dir() -> str:
    """
    The path of a Visual Studio installation's MSVC tools. The result is cached.
    """

    sh.ensure_path_exists(f"{vs_installer_dir()}\\vswhere.exe", kind="file")
    try:
        versions_dir = f"{vs_installation_dir()}\\VC\\Tools\\MSVC"
        newest_version = sorted(
            version
            for version in os.listdir(versions_dir)
            if os.path.isdir(os.path.join(versions_dir, version))
        )[-1]
        ret = f"{versions_dir}\\{newest_version}\\bin\\Hostx64\\x64"

        sh.ensure_path_exists(ret, kind="dir")
    except (sh.CmdException, IndexError, sh.DoesntExistException):
        log.fatal("Couldn't find Visual Studio's MSVC tools.")

    log.info("Visual Studio's MSVC Tools found.")
    return ret
