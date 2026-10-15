use aigate_cli::models::AREAS;
use anyhow::{Context, Result};
use clap::{Arg, Command, ValueEnum, ValueHint};
use serde_json::{json, Value};
use std::fmt::Write;

#[derive(Clone, Copy, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
    #[value(alias = "cmder")]
    Clink,
    #[value(alias = "nu")]
    Nushell,
}

pub fn generate(command: Command, shell: CompletionShell) -> Result<String> {
    // Suggestions belong only to the completion tree. Accept new API values at runtime.
    let mut command = command
        .mut_subcommand("skus", |cmd| {
            cmd.mut_arg("area", |arg| arg.value_parser(AREAS))
        })
        .mut_subcommand("images", |cmd| {
            cmd.mut_arg("area", |arg| arg.value_parser(AREAS))
        })
        .mut_subcommand("create", |cmd| {
            cmd.mut_arg("area", |arg| arg.value_parser(AREAS))
        })
        .mut_subcommand("list", |cmd| {
            cmd.mut_arg("operation_status", |arg| arg.value_parser(["2", "4"]))
        });
    match shell {
        CompletionShell::Clink => {
            command.build();
            return Ok(clink(&command));
        }
        CompletionShell::PowerShell => {
            command.build();
            return Ok(include_str!("completions/powershell.ps1").replace(
                "__AIGATE_SCHEMA__",
                &serde_json::to_string(&schema(&command))?,
            ));
        }
        _ => {}
    }
    let mut script = Vec::new();
    if matches!(shell, CompletionShell::Nushell) {
        clap_complete::generate(
            clap_complete_nushell::Nushell,
            &mut command,
            "aigate-cli",
            &mut script,
        );
    } else {
        let generator = match shell {
            CompletionShell::Bash => clap_complete::Shell::Bash,
            CompletionShell::Zsh => clap_complete::Shell::Zsh,
            CompletionShell::Fish => clap_complete::Shell::Fish,
            _ => unreachable!(),
        };
        // clap_complete 4.6.11 generates inconsistent Bash subcommand paths
        // for hyphenated names. Only rename the final `complete` registration.
        let bin_name = if matches!(shell, CompletionShell::Bash) {
            "aigate_cli"
        } else {
            "aigate-cli"
        };
        clap_complete::generate(generator, &mut command, bin_name, &mut script);
    }
    let mut script = String::from_utf8(script).context("shell 初始化脚本不是 UTF-8")?;
    if matches!(shell, CompletionShell::Nushell) {
        // clap_complete_nushell 4.6.2 omits visible command aliases. Reuse each
        // canonical extern signature and its value completers for alias paths.
        let aliases = nushell_aliases(&command, &script, "aigate-cli", "aigate-cli");
        let end = script
            .rfind("\n}\n")
            .context("Nushell 补全模块缺少结束标记")?;
        script.insert_str(end, &aliases);
    }
    if matches!(shell, CompletionShell::Fish) {
        // clap_complete's Fish generator omits positional value candidates.
        let shells = CompletionShell::value_variants()
            .iter()
            .filter_map(ValueEnum::to_possible_value)
            .map(|value| value.get_name().to_owned())
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(script, "\ncomplete -c aigate-cli -n '__fish_aigate_cli_using_subcommand init; and not __fish_seen_subcommand_from {shells}' -f -a '{shells}'")?;
    }
    if matches!(shell, CompletionShell::Bash) {
        script = script.replace(" aigate_cli\n", " aigate-cli\n");
    }
    Ok(script)
}

fn nushell_aliases(command: &Command, script: &str, canonical: &str, alias: &str) -> String {
    let mut output = String::new();
    for child in command.get_subcommands().filter(|cmd| !cmd.is_hide_set()) {
        let canonical = format!("{canonical} {}", child.get_name());
        for name in child.get_name_and_visible_aliases() {
            let alias = format!("{alias} {name}");
            if alias != canonical {
                let declaration = format!("  export extern \"{canonical}\" [");
                if let Some((_, tail)) = script.split_once(&declaration) {
                    if let Some((signature, _)) = tail.split_once("\n  ]") {
                        writeln!(output, "\n  export extern \"{alias}\" [{signature}\n  ]")
                            .unwrap();
                    }
                }
            }
            output.push_str(&nushell_aliases(child, script, &canonical, &alias));
        }
    }
    output
}

fn values(arg: &Arg) -> Vec<String> {
    arg.get_value_parser()
        .possible_values()
        .into_iter()
        .flatten()
        .filter(|value| !value.is_hide_set())
        .map(|value| value.get_name().to_owned())
        .collect()
}

fn is_path(arg: &Arg) -> bool {
    matches!(
        arg.get_value_hint(),
        ValueHint::AnyPath | ValueHint::FilePath | ValueHint::DirPath
    )
}

fn flags(arg: &Arg) -> Vec<String> {
    arg.get_short_and_visible_aliases()
        .into_iter()
        .flatten()
        .map(|short| format!("-{short}"))
        .chain(
            arg.get_long_and_visible_aliases()
                .into_iter()
                .flatten()
                .map(|long| format!("--{long}")),
        )
        .collect()
}

fn schema(command: &Command) -> Value {
    let options: Vec<_> = command
        .get_arguments()
        .filter(|arg| !arg.is_positional() && !arg.is_hide_set())
        .flat_map(|arg| {
            flags(arg).into_iter().map(|name| {
                json!({"name":name,"takesValue":arg.get_num_args().is_some_and(|n| n.takes_values()),
                    "values":values(arg),"path":is_path(arg),"help":arg.get_help().map(ToString::to_string).unwrap_or_default()})
            })
        })
        .collect();
    let positionals: Vec<_> = command
        .get_positionals()
        .filter(|arg| !arg.is_hide_set())
        .map(|arg| json!({"values":values(arg),"path":is_path(arg)}))
        .collect();
    let subcommands: Vec<_> = command.get_subcommands().filter(|cmd| !cmd.is_hide_set())
        .map(|cmd| json!({"names":cmd.get_name_and_visible_aliases(),"command":schema(cmd),"help":cmd.get_about().map(ToString::to_string).unwrap_or_default()})).collect();
    json!({"options":options,"positionals":positionals,"subcommands":subcommands})
}

fn lua_string(text: &str) -> String {
    let mut result = String::from("\"");
    for ch in text.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            ch if ch.is_control() => write!(result, "\\{:03}", ch as u32).unwrap(),
            _ => result.push(ch),
        }
    }
    result.push('"');
    result
}

fn clink_values(arg: &Arg) -> String {
    if is_path(arg) {
        "{ clink.filematches }".to_owned()
    } else {
        format!(
            "{{ {} }}",
            values(arg)
                .iter()
                .map(|s| lua_string(s))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn clink_command(command: &Command, output: &mut String, index: &mut usize, root: bool) -> String {
    let mut children = Vec::new();
    for subcommand in command.get_subcommands().filter(|cmd| !cmd.is_hide_set()) {
        let parser = clink_command(subcommand, output, index, false);
        for name in subcommand.get_name_and_visible_aliases() {
            children.push(format!("{}..{parser}", lua_string(name)));
        }
    }
    let parser = format!("c{index}");
    *index += 1;
    let registration = if root {
        "\"aigate-cli\", \"aigate-cli.exe\""
    } else {
        ""
    };
    writeln!(
        output,
        "local {parser} = clink.argmatcher({registration}):nofiles()"
    )
    .unwrap();
    if !children.is_empty() {
        writeln!(output, "{parser}:addarg({{ {} }})", children.join(", ")).unwrap();
    }
    for arg in command.get_positionals().filter(|arg| !arg.is_hide_set()) {
        writeln!(output, "{parser}:addarg({})", clink_values(arg)).unwrap();
    }
    let mut options = Vec::new();
    for arg in command
        .get_arguments()
        .filter(|arg| !arg.is_positional() && !arg.is_hide_set())
    {
        for flag in flags(arg) {
            if arg.get_num_args().is_some_and(|n| n.takes_values()) {
                let matcher = format!("clink.argmatcher():addarg({}):nofiles()", clink_values(arg));
                options.push(format!("{}..{matcher}", lua_string(&flag)));
                if flag.starts_with("--") {
                    options.push(format!("{}..{matcher}", lua_string(&format!("{flag}="))));
                }
            } else {
                options.push(lua_string(&flag));
            }
        }
    }
    writeln!(output, "{parser}:addflags({{ {} }})\n", options.join(", ")).unwrap();
    parser
}

fn clink(command: &Command) -> String {
    let mut output = String::from("-- Generated by aigate-cli init clink. Load with Clink 1.x (also used by Cmder).\nif not clink or not clink.argmatcher then return end\n\n");
    clink_command(command, &mut output, &mut 0, true);
    output
}
