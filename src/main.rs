use aigate_cli::auth;
use aigate_cli::client::{normalize_origin, Client, DEFAULT_BASE};
use aigate_cli::models::{parse_positive_id, status_label, CreateRequest, AREAS};
use aigate_cli::output;
use anyhow::{bail, Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

const BUILD_VERSION: &str = match option_env!("AIGATE_BUILD_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Parser)]
#[command(version = BUILD_VERSION, about = "云扉 AIGate 资源与实例管理工具")]
struct Cli {
    /// 输出 JSON（仅 data，不含 API envelope；提示写入 stderr）
    #[arg(long, global = true)]
    json: bool,
    /// API origin；默认连接云扉官方服务
    #[arg(long, global = true, default_value = DEFAULT_BASE)]
    base_url: String,
    /// 单次 HTTP 请求超时秒数
    #[arg(long, global = true, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..=300))]
    timeout: u64,
    /// 忽略系统 HTTP(S)_PROXY / ALL_PROXY
    #[arg(long, global = true)]
    no_proxy: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 隐藏输入 Token 并保存到系统 Keyring
    Login,
    /// 删除系统 Keyring 中的 Token
    Logout,
    /// 输出 shell 补全初始化脚本（离线；由 shell 配置加载）
    Init {
        #[arg(value_enum)]
        shell: CompletionShell,
    },
    /// 列出文档支持的区域（离线）
    Regions,
    /// 查询可创建的 GPU SKU；省略区域时查询两个区域
    #[command(visible_aliases = ["gpus", "stock"])]
    Skus {
        #[arg(long, value_parser = nonempty)]
        area: Option<String>,
    },
    /// 查询实例列表；默认单页，--all 查询全部页
    List {
        #[command(flatten)]
        paging: Paging,
        /// 2=运行中，4=已释放；省略时按 API 默认查询计费中实例
        #[arg(long, value_parser = nonempty)]
        operation_status: Option<String>,
    },
    /// 查询个人或社区镜像
    Images(ImageArgs),
    /// 查询实例详情；默认隐藏密码、Token、工具配置与 URL 查询串
    Info {
        #[arg(value_parser = positive_id)]
        instance_id: i64,
        /// 显示连接凭证，仅在私密终端使用
        #[arg(long)]
        show_secrets: bool,
    },
    /// 查看计费中实例状态总览；指定 ID 时查询单个实例
    Status {
        /// 省略时查询全部计费中实例（自动读取全部分页）
        #[arg(value_parser = positive_id)]
        instance_id: Option<i64>,
    },
    /// 创建并启动新实例（可能立即计费；必须确认）
    Create(CreateArgs),
    /// 启动指定已有实例（仅状态 22/26）
    Start {
        #[arg(value_parser = positive_id)]
        instance_id: i64,
    },
    /// 关闭指定实例（仅状态 2）
    Stop {
        #[arg(value_parser = positive_id)]
        instance_id: i64,
    },
    /// 释放指定实例（删除资源；必须确认）
    #[command(visible_alias = "rm")]
    Release {
        #[arg(value_parser = positive_id)]
        instance_id: i64,
        /// 明确确认释放该 ID；允许非交互使用
        #[arg(long, short = 'y', conflicts_with = "dry_run")]
        yes: bool,
        /// 离线输出将发送的请求，不执行释放
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

fn init_shell(shell: CompletionShell) -> Result<()> {
    let generator = match shell {
        CompletionShell::Bash => clap_complete::Shell::Bash,
        CompletionShell::Zsh => clap_complete::Shell::Zsh,
        CompletionShell::Fish => clap_complete::Shell::Fish,
    };
    // Add suggestions to the completion schema only; runtime area/status
    // validation remains forward-compatible with new values from the API.
    let mut command = Cli::command()
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
    let mut script = Vec::new();
    // clap_complete 4.6.11 builds inconsistent Bash subcommand paths for
    // hyphenated binary names. Use an underscore internally and rename only
    // the command registered with `complete` in the generated script below.
    let bin_name = if matches!(shell, CompletionShell::Bash) {
        "aigate_cli"
    } else {
        "aigate-cli"
    };
    clap_complete::generate(generator, &mut command, bin_name, &mut script);
    if matches!(shell, CompletionShell::Fish) {
        // clap_complete's Fish generator does not emit positional values.
        let shells = CompletionShell::value_variants()
            .iter()
            .filter_map(ValueEnum::to_possible_value)
            .map(|value| value.get_name().to_owned())
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(
            &mut script,
            "\ncomplete -c aigate-cli -n '__fish_aigate_cli_using_subcommand init; and not __fish_seen_subcommand_from {shells}' -f -a '{shells}'"
        )
        .context("无法生成 Fish shell 名称补全")?;
    }
    let script = String::from_utf8(script).context("shell 初始化脚本不是 UTF-8")?;
    let script = if matches!(shell, CompletionShell::Bash) {
        script.replace(" aigate_cli\n", " aigate-cli\n")
    } else {
        script
    };
    io::stdout()
        .lock()
        .write_all(script.as_bytes())
        .context("无法输出 shell 初始化脚本")
}

#[derive(Args)]
struct Paging {
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..), conflicts_with = "all")]
    page: u32,
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u8).range(1..=20))]
    size: u8,
    /// 从第 1 页查询到 total，不静默截断结果
    #[arg(long)]
    all: bool,
}

#[derive(Args)]
struct ImageArgs {
    /// 3=个人镜像，2=社区镜像；社区镜像须指定 --area、--sku 和 --name
    #[arg(long, default_value = "3", value_parser = ["2", "3"], requires_ifs = [("2", "area"), ("2", "sku"), ("2", "name")])]
    image_type: String,
    #[arg(long, value_parser = nonempty)]
    area: Option<String>,
    #[arg(long, value_parser = nonempty)]
    sku: Option<String>,
    /// 社区镜像名称过滤；查询社区镜像时必填
    #[arg(long, value_parser = nonempty)]
    name: Option<String>,
    /// 社区镜像版本过滤
    #[arg(long, value_parser = nonempty)]
    image_version: Option<String>,
    #[command(flatten)]
    paging: Paging,
}

#[derive(Args)]
#[command(group(clap::ArgGroup::new("gpu").required(true).args(["sku", "gpu_model"])))]
struct CreateArgs {
    /// SKU 名称，与 --gpu-model 二选一
    #[arg(long, value_parser = nonempty)]
    sku: Option<String>,
    #[arg(long, value_parser = nonempty)]
    gpu_model: Option<String>,
    #[arg(long, value_parser = nonempty)]
    area: String,
    #[arg(long, value_parser = positive_id)]
    image_id: i64,
    /// 2=社区镜像，3=个人镜像；必须明确选择
    #[arg(long, value_parser = ["2", "3"])]
    image_type: String,
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=8))]
    count: u8,
    /// 明确确认创建计费资源；允许非交互使用
    #[arg(long, short = 'y', conflicts_with = "dry_run")]
    yes: bool,
    /// 离线输出将发送的请求，不创建或计费
    #[arg(long)]
    dry_run: bool,
}

fn nonempty(raw: &str) -> std::result::Result<String, String> {
    let value = raw.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err("必须为非空文本且不含控制字符".into());
    }
    Ok(value.into())
}

fn positive_id(raw: &str) -> std::result::Result<i64, String> {
    parse_positive_id(raw).ok_or_else(|| "ID 必须为正整数（int64）".into())
}

impl Cli {
    fn validate(&self) -> std::result::Result<(), clap::Error> {
        if !matches!(&self.command, Command::Init { .. } | Command::Regions) {
            if let Err(error) = normalize_origin(&self.base_url) {
                // Use only the fixed diagnostic; never echo credentials embedded
                // in a rejected URL, including for offline previews.
                return Err(Self::command().error(clap::error::ErrorKind::ValueValidation, error));
            }
        }
        if let Command::Images(args) = &self.command {
            let message = if args.paging.size < 5 {
                Some("镜像分页 --size 必须为 5–20")
            } else if args.image_type == "3"
                && (args.area.is_some()
                    || args.sku.is_some()
                    || args.name.is_some()
                    || args.image_version.is_some())
            {
                Some("--area/--sku/--name/--image-version 仅用于 --image-type 2 社区镜像")
            } else {
                None
            };
            if let Some(message) = message {
                return Err(Self::command().error(clap::error::ErrorKind::ValueValidation, message));
            }
        }
        Ok(())
    }
}

fn confirm(expected: &str, yes: bool) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        bail!("非交互模式拒绝执行；请审阅 --dry-run 后使用 --yes 明确确认");
    }
    eprint!("输入 {expected} 确认，其余输入取消: ");
    io::stderr().flush().context("无法输出确认提示")?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("无法读取确认输入")?;
    if answer.trim() != expected {
        bail!("操作已取消，未发送请求");
    }
    Ok(())
}

fn client(cli: &Cli) -> Result<Client> {
    let base = normalize_origin(&cli.base_url)?;
    Client::new(
        auth::resolve(&base)?,
        base.as_str(),
        Duration::from_secs(cli.timeout),
        cli.no_proxy,
    )
}

fn emit(mut value: Value, show_secrets: bool) -> Result<()> {
    if !show_secrets {
        output::redact(&mut value);
    }
    output::json(&value)
}

fn accepted(cli: &Cli, action: &str, id: i64) -> Result<()> {
    if cli.json {
        emit(
            json!({"action": action, "instanceId": id.to_string(), "accepted": true}),
            false,
        )
    } else {
        println!("{action} 请求已接受：{id}；用 `aigate-cli status {id}` 查询最终状态");
        Ok(())
    }
}

async fn run(cli: Cli) -> Result<()> {
    match &cli.command {
        Command::Login => {
            auth::require_keyring_origin(&normalize_origin(&cli.base_url)?)?;
            auth::login()
        }
        Command::Logout => {
            auth::require_keyring_origin(&normalize_origin(&cli.base_url)?)?;
            auth::logout()
        }
        Command::Init { shell } => init_shell(*shell),
        Command::Regions => {
            if cli.json {
                emit(json!(AREAS), false)
            } else {
                for area in AREAS {
                    println!("{area}");
                }
                Ok(())
            }
        }
        Command::Skus { area } => {
            let client = client(&cli)?;
            let areas: Vec<&str> = area
                .as_deref()
                .map(|a| vec![a])
                .unwrap_or_else(|| AREAS.to_vec());
            let mut records = Vec::new();
            for area in areas {
                records.extend(client.skus(area).await?);
            }
            if cli.json {
                emit(json!(records), false)
            } else {
                output::table(
                    &records,
                    &[
                        ("区域", "areaName"),
                        ("SKU", "skuName"),
                        ("价格(原值)", "price"),
                        ("CPU核", "cpuCore"),
                        ("内存(原值)", "memorySize"),
                        ("系统盘(原值)", "sysDiskSize"),
                        ("最多GPU", "maxGpuCount"),
                    ],
                );
                Ok(())
            }
        }
        Command::List {
            paging,
            operation_status,
        } => {
            let page = client(&cli)?
                .instances(
                    paging.page,
                    paging.size,
                    operation_status.as_deref(),
                    paging.all,
                )
                .await?;
            if cli.json {
                emit(serde_json::to_value(page)?, false)
            } else {
                output::table(
                    &page.records,
                    &[
                        ("ID", "instanceId"),
                        ("名称", "instanceName"),
                        ("状态", "operationStatus"),
                        ("区域", "areaName"),
                        ("GPU数", "count"),
                        ("镜像ID", "imageId"),
                        ("镜像类型", "imageType"),
                    ],
                );
                eprintln!(
                    "显示 {} 条，API total={}{}",
                    page.records.len(),
                    page.total,
                    if paging.all {
                        ""
                    } else {
                        "；可加 --all 获取全部页"
                    }
                );
                Ok(())
            }
        }
        Command::Images(args) => {
            let mut body = json!({"current": args.paging.page, "pageSize": args.paging.size, "imageType": args.image_type});
            if args.image_type == "2" {
                body["areaName"] = json!(args.area);
                body["skuName"] = json!(args.sku);
                body["imageName"] = json!(args.name.as_deref().unwrap_or(""));
                body["imageVersion"] = json!(args.image_version.as_deref().unwrap_or(""));
            }
            let page = client(&cli)?.images(body, args.paging.all).await?;
            if cli.json {
                emit(serde_json::to_value(page)?, false)
            } else {
                output::table(
                    &page.records,
                    &[
                        ("镜像ID", "worksId"),
                        ("名称", "name"),
                        ("区域", "areaName"),
                        ("类型", "imageType"),
                        ("状态(原值)", "status"),
                        ("版本", "imageVersion"),
                        ("大小(byte)", "size"),
                    ],
                );
                eprintln!(
                    "显示 {} 条，API total={}{}",
                    page.records.len(),
                    page.total,
                    if args.paging.all {
                        ""
                    } else {
                        "；可加 --all 获取全部页"
                    }
                );
                Ok(())
            }
        }
        Command::Info {
            instance_id,
            show_secrets,
        } => emit(client(&cli)?.info(*instance_id).await?, *show_secrets),
        Command::Status {
            instance_id: Some(instance_id),
        } => {
            let code = client(&cli)?.status(*instance_id).await?;
            if cli.json {
                emit(
                    json!({"instanceId": instance_id.to_string(), "status": code, "label": status_label(&code)}),
                    false,
                )
            } else {
                output::status(*instance_id, &code);
                Ok(())
            }
        }
        Command::Status { instance_id: None } => {
            let overview = client(&cli)?.status_overview().await?;
            if cli.json {
                emit(serde_json::to_value(overview)?, false)
            } else {
                output::status_overview(&overview);
                Ok(())
            }
        }
        Command::Create(args) => {
            let request = CreateRequest {
                sku_name: args.sku.clone(),
                gpu_model: args.gpu_model.clone(),
                area_name: args.area.clone(),
                count: args.count,
                image_id: args.image_id,
                image_type: args.image_type.clone(),
            };
            if args.dry_run {
                return emit(
                    json!({"dryRun": true, "method": "POST", "origin": cli.base_url, "path": "/api/openapi/instance/start", "body": request}),
                    false,
                );
            }
            eprintln!(
                "即将在 {} 创建并启动新实例，可能立即计费：\n{}",
                cli.base_url,
                serde_json::to_string_pretty(&request)?
            );
            confirm("create", args.yes)?;
            let data = client(&cli)?.create(&request).await?;
            if !cli.json {
                eprintln!("创建请求已接受；请用 status 查询最终状态");
            }
            emit(data, false)
        }
        Command::Start { instance_id } => {
            client(&cli)?.start(*instance_id).await?;
            accepted(&cli, "start", *instance_id)
        }
        Command::Stop { instance_id } => {
            client(&cli)?.stop(*instance_id).await?;
            accepted(&cli, "stop", *instance_id)
        }
        Command::Release {
            instance_id,
            yes,
            dry_run,
        } => {
            if *dry_run {
                return emit(
                    json!({"dryRun": true, "method": "GET", "origin": cli.base_url, "path": "/api/openapi/instance/release", "query": {"instanceId": instance_id.to_string()}}),
                    false,
                );
            }
            eprintln!(
                "即将在 {} 释放实例 {instance_id}，实例资源会被删除，请确认已备份所需数据。",
                cli.base_url
            );
            confirm(&instance_id.to_string(), *yes)?;
            client(&cli)?.release(*instance_id).await?;
            accepted(&cli, "release", *instance_id)
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    if let Err(error) = cli.validate() {
        error.exit();
    }
    if let Err(error) = run(cli).await {
        eprintln!("错误：{error:#}");
        std::process::exit(1);
    }
}
