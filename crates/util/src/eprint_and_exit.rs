//! Defines [eprint_and_exit] and [eprintln_and_exit].

/// The equivalent to calling [eprint], then calling [std::process::exit] with
/// with an exit code of `1`.
///
/// Useful for exiting gracefully with an error message.
#[macro_export]
macro_rules! eprint_and_exit {
    ($($arg:tt)*) => {{
        eprint!($($arg)*);
        ::std::process::exit(1);
    }};
}

/// The equivalent to calling [eprintln], then calling [std::process::exit] with
/// with an exit code of `1`.
///
/// Useful for exiting gracefully with an error message.
#[macro_export]
macro_rules! eprintln_and_exit {
    ($($arg:tt)*) => {{
        eprintln!($($arg)*);
        ::std::process::exit(1);
    }};
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    // Helper: re-invokes the current test binary as a subprocess running a
    // specific "exit test" function, then returns the output.
    fn run_exit_test(test_name: &str) -> std::process::Output {
        let current_exe = std::env::current_exe().expect("could not get current exe path");
        Command::new(current_exe)
            .args(["--test-threads=1", "--nocapture", "--ignored", test_name])
            .output()
            .expect("failed to run subprocess")
    }

    // --- eprint_and_exit! ---
    // Decision 1: eprint fires => output appears on stderr
    // Decision 2: exit(1) fires => process exits with code 1

    #[test]
    #[ignore = "run as subprocess only"]
    fn eprint_and_exit_subprocess() {
        crate::eprint_and_exit!("eprint_and_exit fired: {}", 42);
    }

    #[test]
    fn eprint_and_exit_exits_with_code_1() {
        let output = run_exit_test("eprint_and_exit_subprocess");
        assert_eq!(
            output.status.code(),
            Some(1),
            "expected exit code 1, got: {:?}",
            output.status
        );
    }

    #[test]
    fn eprint_and_exit_writes_message_to_stderr() {
        let output = run_exit_test("eprint_and_exit_subprocess");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("eprint_and_exit fired: 42"),
            "expected message in stderr, got: {stderr:?}"
        );
    }

    #[test]
    fn eprint_and_exit_writes_nothing_to_stdout() {
        let output = run_exit_test("eprint_and_exit_subprocess");
        // eprint goes to stderr only; stdout should have no user message
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("eprint_and_exit fired"),
            "unexpected message in stdout: {stdout:?}"
        );
    }

    // --- eprintln_and_exit! ---
    // Decision 1: eprintln fires => output appears on stderr with trailing newline
    // Decision 2: exit(1) fires => process exits with code 1

    #[test]
    #[ignore = "run as subprocess only"]
    fn eprintln_and_exit_subprocess() {
        crate::eprintln_and_exit!("eprintln_and_exit fired: {}", 99);
    }

    #[test]
    fn eprintln_and_exit_exits_with_code_1() {
        let output = run_exit_test("eprintln_and_exit_subprocess");
        assert_eq!(
            output.status.code(),
            Some(1),
            "expected exit code 1, got: {:?}",
            output.status
        );
    }

    #[test]
    fn eprintln_and_exit_writes_message_to_stderr() {
        let output = run_exit_test("eprintln_and_exit_subprocess");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("eprintln_and_exit fired: 99"),
            "expected message in stderr, got: {stderr:?}"
        );
    }

    #[test]
    fn eprintln_and_exit_appends_newline_to_stderr() {
        let output = run_exit_test("eprintln_and_exit_subprocess");
        let stderr = String::from_utf8_lossy(&output.stderr);
        // eprintln adds \n; eprint does not — verify the newline is present
        assert!(
            stderr.contains("eprintln_and_exit fired: 99\n"),
            "expected trailing newline in stderr, got: {stderr:?}"
        );
    }

    #[test]
    fn eprintln_and_exit_writes_nothing_to_stdout() {
        let output = run_exit_test("eprintln_and_exit_subprocess");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("eprintln_and_exit fired"),
            "unexpected message in stdout: {stdout:?}"
        );
    }

    // --- eprint vs eprintln distinction ---
    // Decision: eprint does NOT add a newline; eprintln DOES

    #[test]
    #[ignore = "run as subprocess only"]
    fn eprint_no_newline_subprocess() {
        crate::eprint_and_exit!("no newline here");
    }

    #[test]
    fn eprint_and_exit_does_not_append_newline() {
        let output = run_exit_test("eprint_no_newline_subprocess");
        let stderr = String::from_utf8_lossy(&output.stderr);
        // The message itself has no \n — if it ends with \n that's eprintln behavior
        assert!(
            stderr.contains("no newline here"),
            "message missing from stderr: {stderr:?}"
        );
        assert!(
            !stderr.contains("no newline here\n"),
            "eprint should not add a trailing newline, but got: {stderr:?}"
        );
    }
}
