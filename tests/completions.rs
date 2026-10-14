#![cfg(unix)]

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn script(shell: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_aigate-cli"))
        .args(["--base-url", "invalid-origin", "init", shell])
        .env("AIGATE_TOKEN", "")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn available(shell: &str) -> bool {
    let reason = match Command::new(shell).arg("--version").output() {
        Ok(output) if output.status.success() => return true,
        Ok(output) => format!("--version exited with {}", output.status),
        Err(error) => error.to_string(),
    };
    assert!(
        std::env::var("AIGATE_REQUIRE_SHELLS").as_deref() != Ok("1"),
        "{shell} is required for shell execution checks: {reason}"
    );
    eprintln!("SKIPPED {shell} execution check: {reason}; set AIGATE_REQUIRE_SHELLS=1 to require all shells");
    false
}

fn run_shell(shell: &str, args: &[&str], input: &str) -> Output {
    let mut child = Command::new(shell)
        .args(args)
        .env("AIGATE_TOKEN", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{shell}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{shell}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_candidates(output: &Output, expected: &[&str]) {
    let text = String::from_utf8_lossy(&output.stdout);
    let candidates: Vec<_> = text
        .lines()
        .map(|line| line.split('\t').next().unwrap())
        .collect();
    for candidate in expected {
        assert!(
            candidates.contains(candidate),
            "missing {candidate:?} in {candidates:?}"
        );
    }
}

#[test]
fn bash_loads_init_and_completes_commands_options_and_values() {
    if !available("bash") {
        return;
    }
    let init = script("bash");
    for (words, cword, expected) in [
        (
            "aigate-cli ''",
            1,
            vec!["create", "gpus", "stock", "rm", "init"],
        ),
        (
            "aigate-cli create --image-",
            2,
            vec!["--image-id", "--image-type"],
        ),
        (
            "aigate-cli create --area 华",
            3,
            vec!["华东一区", "华东二区"],
        ),
        ("aigate-cli images --image-type ''", 3, vec!["2", "3"]),
        ("aigate-cli list --operation-status ''", 3, vec!["2", "4"]),
        ("aigate-cli init ''", 2, vec!["bash", "zsh", "fish"]),
        (
            "aigate-cli workflow ''",
            2,
            vec!["login", "run", "status", "upload"],
        ),
        ("aigate-cli comfy run --instance-", 3, vec!["--instance-id"]),
        (
            "aigate-cli workflow upload --media-type ''",
            4,
            vec!["image", "video", "audio"],
        ),
    ] {
        let input = format!(
            r#"{init}
COMP_WORDS=({words})
COMP_CWORD={cword}
COMP_LINE="${{COMP_WORDS[*]}}"
COMP_POINT=${{#COMP_LINE}}
completion_registration=$(complete -p aigate-cli)
completion_function=${{completion_registration#* -F }}
completion_function=${{completion_function%% *}}
"$completion_function" "${{COMP_WORDS[0]}}" "${{COMP_WORDS[COMP_CWORD]}}" "${{COMP_WORDS[COMP_CWORD-1]}}"
printf '%s\n' "${{COMPREPLY[@]}}"
"#
        );
        let output = run_shell("bash", &["--noprofile", "--norc"], &input);
        assert_candidates(&output, &expected);
    }
}

#[test]
fn fish_loads_init_and_completes_commands_options_and_values() {
    if !available("fish") {
        return;
    }
    let init = script("fish");
    for (line, expected) in [
        ("aigate-cli ", vec!["create", "gpus", "stock", "rm", "init"]),
        (
            "aigate-cli create --image-",
            vec!["--image-id", "--image-type"],
        ),
        ("aigate-cli create --area 华", vec!["华东一区", "华东二区"]),
        ("aigate-cli images --image-type ", vec!["2", "3"]),
        ("aigate-cli list --operation-status ", vec!["2", "4"]),
        ("aigate-cli init ", vec!["bash", "zsh", "fish"]),
        (
            "aigate-cli workflow ",
            vec!["login", "run", "status", "upload"],
        ),
        ("aigate-cli comfy run --instance-", vec!["--instance-id"]),
        (
            "aigate-cli workflow upload --media-type ",
            vec!["image", "video", "audio"],
        ),
    ] {
        let input = format!("{init}\ncomplete --do-complete '{line}'\n");
        let output = run_shell("fish", &["--no-config"], &input);
        assert_candidates(&output, &expected);
    }
}

#[test]
fn zsh_loads_init_and_registers_command_completion() {
    if !available("zsh") {
        return;
    }
    let input = format!(
        "autoload -Uz compinit\ncompinit -i -D\n{}\n[[ -n ${{_comps[aigate-cli]}} ]] || exit 1\n(( $+functions[${{_comps[aigate-cli]}}] )) || exit 1\nprint -r -- ${{_comps[aigate-cli]}}\n",
        script("zsh")
    );
    let output = run_shell("zsh", &["-f"], &input);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "_aigate-cli"
    );
}
