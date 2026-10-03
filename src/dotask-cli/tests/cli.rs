use std::process::{Command, Output};

fn run(arguments: &[&str]) -> Output {
    // No language host or installed dotask may satisfy these smoke tests.
    Command::new(env!("CARGO_BIN_EXE_dotask"))
        .args(arguments)
        .env("PATH", "")
        .output()
        .expect("launch the native CLI")
}

#[test]
fn help_runs_without_an_external_language_host() {
    for arguments in [
        &["--help"][..],
        &["-h"],
        &["--use-dir", "missing", "--help"],
    ] {
        let output = run(arguments);
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("Usage: dotask"));
        assert!(stdout.contains("Runs installed C# tasks"));
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn version_identifies_the_development_preview() {
    let output = run(&["--version"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        format!(
            "dotask {} (Rust CLI development preview)",
            env!("CARGO_PKG_VERSION")
        )
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn unimplemented_requests_fail_instead_of_reporting_success() {
    for arguments in [
        &["--init"][..],
        &["--add", "git", "--lang", "rust"],
        &["completion", "powershell"],
        &["__complete"],
    ] {
        let output = run(arguments);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("existing C# CLI")
        );
    }
}
