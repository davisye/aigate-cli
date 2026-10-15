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
    let required = match shell {
        "pwsh" => "AIGATE_REQUIRE_POWERSHELL",
        "nu" => "AIGATE_REQUIRE_NUSHELL",
        "lua" => "AIGATE_REQUIRE_LUA",
        _ => "AIGATE_REQUIRE_SHELLS",
    };
    let version = if shell == "lua" { "-v" } else { "--version" };
    let reason = match Command::new(shell).arg(version).output() {
        Ok(output) if output.status.success() => return true,
        Ok(output) => format!("--version exited with {}", output.status),
        Err(error) => error.to_string(),
    };
    assert!(
        std::env::var(required).as_deref() != Ok("1"),
        "{shell} is required for shell execution checks: {reason}"
    );
    eprintln!("SKIPPED {shell} execution check: {reason}; set {required}=1 to require this shell");
    false
}

#[cfg(unix)]
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
#[cfg(unix)]
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
        (
            "aigate-cli init ''",
            2,
            vec!["bash", "zsh", "fish", "powershell", "clink", "nushell"],
        ),
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
#[cfg(unix)]
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
        (
            "aigate-cli init ",
            vec!["bash", "zsh", "fish", "powershell", "clink", "nushell"],
        ),
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
#[cfg(unix)]
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

fn run_script(shell: &str, args: &[&str], extension: &str, input: &str) -> Output {
    let mut file = tempfile::Builder::new()
        .suffix(extension)
        .tempfile()
        .unwrap();
    // PowerShell on Windows must read the Chinese suggestions as UTF-8.
    if extension == ".ps1" {
        file.write_all(b"\xef\xbb\xbf").unwrap();
    }
    file.write_all(input.as_bytes()).unwrap();
    let output = Command::new(shell)
        .args(args)
        .arg(file.path())
        .env("AIGATE_TOKEN", "")
        .env("AIGATE_WORKFLOW_TOKEN", "")
        .stdin(Stdio::null())
        .output()
        .unwrap();
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

#[test]
fn powershell_loads_init_and_completes_nested_commands_options_and_values() {
    if !available("pwsh") {
        return;
    }
    let init = script("powershell");
    for (line, expected) in [
        (
            "aigate-cli ",
            vec!["workflow", "comfy", "gpus", "stock", "rm", "init"],
        ),
        (
            "aigate-cli.exe workflow ",
            vec!["login", "run", "status", "upload"],
        ),
        (
            "aigate-cli --json --timeout 60 comfy run --instance-",
            vec!["--instance-id"],
        ),
        (
            "aigate-cli create --image-",
            vec!["--image-id", "--image-type"],
        ),
        ("aigate-cli create --area 华", vec!["华东一区", "华东二区"]),
        (
            "aigate-cli gpus --area=华",
            vec!["--area=华东一区", "--area=华东二区"],
        ),
        ("aigate-cli images --image-type ", vec!["2", "3"]),
        ("aigate-cli list --operation-status ", vec!["2", "4"]),
        (
            "aigate-cli workflow upload 'input file.png' --media-type ",
            vec!["image", "video", "audio"],
        ),
        (
            "aigate-cli init ",
            vec!["bash", "zsh", "fish", "powershell", "clink", "nushell"],
        ),
    ] {
        let line = line.replace('\'', "''");
        let input = format!("$ErrorActionPreference = 'Stop'\n{init}\n$line = '{line}'\n[System.Management.Automation.CommandCompletion]::CompleteInput($line, $line.Length, $null).CompletionMatches | ForEach-Object {{ $_.CompletionText }}\n");
        let output = run_script(
            "pwsh",
            &["-NoLogo", "-NoProfile", "-NonInteractive", "-File"],
            ".ps1",
            &input,
        );
        assert_candidates(&output, &expected);
    }
}

#[test]
fn powershell_completes_file_paths_with_spaces_and_unicode() {
    if !available("pwsh") {
        return;
    }
    let directory = tempfile::Builder::new()
        .prefix("aigate 中文 paths ")
        .tempdir()
        .unwrap();
    let file = directory.path().join("input file.json");
    std::fs::write(&file, "{}").unwrap();
    let prefix = directory
        .path()
        .join("input")
        .to_string_lossy()
        .replace('\'', "''");
    let input = format!(
        "$ErrorActionPreference = 'Stop'\n{}\n$line = 'aigate-cli workflow run --input ''{prefix}'\n[System.Management.Automation.CommandCompletion]::CompleteInput($line, $line.Length, $null).CompletionMatches | ForEach-Object {{ $_.CompletionText }}\n",
        script("powershell")
    );
    let output = run_script(
        "pwsh",
        &["-NoLogo", "-NoProfile", "-NonInteractive", "-File"],
        ".ps1",
        &input,
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("input file.json"),
        "missing filename in {text:?}"
    );
    assert!(
        text.contains("aigate 中文 paths "),
        "missing Unicode directory in {text:?}"
    );
}

#[test]
fn nushell_loads_init_and_completes_aliases_nested_commands_options_and_values() {
    if !available("nu") {
        return;
    }
    let init = script("nushell");
    // Nu's IDE protocol returns JSON completions without executing the resource command.
    for (line, expected) in [
        ("aigate-cli wor", vec!["aigate-cli workflow"]),
        (
            "aigate-cli workflow ",
            vec!["aigate-cli workflow run", "aigate-cli workflow upload"],
        ),
        ("aigate-cli comfy run --instance-", vec!["--instance-id"]),
        ("aigate-cli create --area 华", vec!["华东一区", "华东二区"]),
        ("aigate-cli gpus --area 华", vec!["华东一区", "华东二区"]),
        ("aigate-cli images --image-type ", vec!["2", "3"]),
        ("aigate-cli list --operation-status ", vec!["2", "4"]),
        (
            "aigate-cli workflow upload --media-type ",
            vec!["image", "video", "audio"],
        ),
        (
            "aigate-cli init ",
            vec!["bash", "zsh", "fish", "powershell", "clink", "nushell"],
        ),
    ] {
        let input = format!("{init}\n{line}");
        let offset = input.len().to_string();
        let output = run_script(
            "nu",
            &[
                "--no-config-file",
                "--no-history",
                "--ide-complete",
                &offset,
            ],
            ".nu",
            &input,
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let candidates: Vec<_> = result["completions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().or_else(|| value["value"].as_str()))
            .collect();
        for value in expected {
            assert!(
                candidates.contains(&value),
                "missing {value:?} in {candidates:?}; response: {result}"
            );
        }
    }
}

#[test]
fn clink_lua_loads_and_links_commands_flags_values_and_paths() {
    if !available("lua") {
        return;
    }
    let input = format!(
        "{}\n{}\n{}",
        include_str!("support/clink.lua"),
        script("clink"),
        include_str!("support/clink_checks.lua")
    );
    run_script("lua", &[], ".lua", &input);
}
