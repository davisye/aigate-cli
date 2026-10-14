use super::{confirm, emit, nonempty, positive_id, Cli};
use aigate_cli::auth;
use aigate_cli::client::normalize_origin;
use aigate_cli::generation::{self, ComfyClient, MediaType, UploadFile, WorkflowClient};
use aigate_cli::output;
use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Args)]
pub struct WorkflowArgs {
    #[command(subcommand)]
    command: WorkflowCommand,
}

#[derive(Subcommand)]
enum WorkflowCommand {
    /// 保存 Studio Token 到独立的系统 Keyring 项
    Login,
    /// 删除 Workflow 的 Keyring 凭证
    Logout,
    /// 提交生成任务（消耗积分；须明确确认；排队后无法取消）
    Run {
        #[arg(long, value_parser = nonempty)]
        app_id: String,
        /// 工作流输入参数 JSON object 文件
        #[arg(long)]
        input: PathBuf,
        #[command(flatten)]
        submit: Submit,
    },
    /// 查询任务状态及结果文件（JSON）
    Status(TaskStatus),
    /// 上传图片、视频或音频，返回可填入工作流参数的文件结果
    Upload {
        file: PathBuf,
        #[arg(long, value_enum)]
        media_type: Media,
        #[arg(long)]
        dry_run: bool,
        /// 显示含签名的文件 URL，仅在私密终端使用
        #[arg(long, conflicts_with = "dry_run")]
        show_secrets: bool,
    },
}

#[derive(Args)]
pub struct ComfyArgs {
    #[command(subcommand)]
    command: ComfyCommand,
}

#[derive(Subcommand)]
enum ComfyCommand {
    /// 查询已配置的应用；--json 包含输入参数定义
    Apps,
    /// 查询指定应用的实例（使用 Comfy instanceStatus）
    Instances {
        #[arg(value_parser = positive_id)]
        app_id: i64,
    },
    /// 向指定应用和实例提交任务（可能消耗资源；须明确确认）
    Run {
        #[arg(long, value_parser = positive_id)]
        app_id: i64,
        #[arg(long, value_parser = positive_id)]
        instance_id: i64,
        /// 应用 allowInput JSON object 文件
        #[arg(long)]
        input: PathBuf,
        #[command(flatten)]
        submit: Submit,
    },
    /// 查询任务状态及结果文件（JSON）
    Status(TaskStatus),
    /// 上传图片到指定运行中实例的 input 目录
    Upload {
        file: PathBuf,
        #[arg(long, value_parser = positive_id)]
        app_id: i64,
        #[arg(long, value_parser = positive_id)]
        instance_id: i64,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Args)]
struct Submit {
    /// 离线预览请求，不读取凭证或提交任务
    #[arg(long)]
    dry_run: bool,
    /// 明确确认本次生成任务；允许非交互执行
    #[arg(long, short = 'y', conflicts_with = "dry_run")]
    yes: bool,
}

#[derive(Args)]
struct TaskStatus {
    task_id: String,
    /// 显示含签名的结果 URL，仅在私密终端使用
    #[arg(long)]
    show_secrets: bool,
}

pub fn validate(command: &super::Command) -> Result<()> {
    use super::Command;
    let args = match command {
        Command::Workflow(WorkflowArgs {
            command: WorkflowCommand::Status(args),
        })
        | Command::Comfy(ComfyArgs {
            command: ComfyCommand::Status(args),
        }) => args,
        _ => return Ok(()),
    };
    if !generation::valid_task_id(&args.task_id) {
        anyhow::bail!(
            "taskId 必须为有效标识，仅含 ASCII 字母、数字、下划线或连字符，不能为 URL 或占位值"
        );
    }
    Ok(())
}

#[derive(Clone, Copy, ValueEnum)]
enum Media {
    Image,
    Video,
    Audio,
}

impl From<Media> for MediaType {
    fn from(media: Media) -> Self {
        match media {
            Media::Image => Self::Image,
            Media::Video => Self::Video,
            Media::Audio => Self::Audio,
        }
    }
}

fn workflow_client(cli: &Cli) -> Result<WorkflowClient> {
    let base = normalize_origin(cli.origin())?;
    WorkflowClient::new(
        auth::resolve_for(auth::Profile::Workflow, &base)?,
        base.as_str(),
        Duration::from_secs(cli.timeout),
        cli.no_proxy,
    )
}

fn comfy_client(cli: &Cli) -> Result<ComfyClient> {
    let base = normalize_origin(cli.origin())?;
    ComfyClient::new(
        auth::resolve(&base)?,
        base.as_str(),
        Duration::from_secs(cli.timeout),
        cli.no_proxy,
    )
}

fn preview(cli: &Cli, path: &str, body: Value) -> Result<()> {
    emit(
        json!({"dryRun":true,"method":"POST","origin":cli.origin(),"path":path,"body":body}),
        false,
    )
}

fn confirm_task(cli: &Cli, service: &str, body: &Value, yes: bool) -> Result<()> {
    let mut safe = body.clone();
    output::redact(&mut safe);
    eprintln!(
        "即将在 {} 提交 {service} 生成任务，可能消耗积分或实例资源。排队任务可能无法取消。\n{}",
        cli.origin(),
        serde_json::to_string_pretty(&safe)?
    );
    confirm("submit", yes)
}

fn task_accepted(data: Value) -> Result<()> {
    eprintln!("任务提交已接受；请用返回的 taskId 查询最终状态");
    emit(data, false)
}

pub async fn workflow(cli: &Cli, args: &WorkflowArgs) -> Result<()> {
    match &args.command {
        WorkflowCommand::Login | WorkflowCommand::Logout => {
            auth::Profile::Workflow.require_keyring_origin(&normalize_origin(cli.origin())?)?;
            if matches!(args.command, WorkflowCommand::Login) {
                auth::login_for(auth::Profile::Workflow)
            } else {
                auth::logout_for(auth::Profile::Workflow)
            }
        }
        WorkflowCommand::Run {
            app_id,
            input,
            submit,
        } => {
            let input = generation::read_input(input)?;
            let body = json!({"appId":app_id,"inputParams":input});
            if submit.dry_run {
                return preview(
                    cli,
                    &format!("{}/create", generation::WORKFLOW_PREFIX),
                    body,
                );
            }
            confirm_task(cli, "Workflow", &body, submit.yes)?;
            task_accepted(workflow_client(cli)?.run(app_id, &input).await?)
        }
        WorkflowCommand::Status(args) => emit(
            workflow_client(cli)?.status(&args.task_id).await?,
            args.show_secrets,
        ),
        WorkflowCommand::Upload {
            file,
            media_type,
            dry_run,
            show_secrets,
        } => {
            let media = MediaType::from(*media_type);
            let file = UploadFile::inspect(file, Some(media))?;
            if *dry_run {
                return emit(
                    json!({"dryRun":true,"method":"POST","origin":cli.origin(),"path":format!("{}/uploadFile",generation::WORKFLOW_PREFIX),"query":{"media_type":media.as_str()},"multipart":{"file":{"name":file.name,"bytes":file.bytes}}}),
                    false,
                );
            }
            emit(
                workflow_client(cli)?.upload(&file, media).await?,
                *show_secrets,
            )
        }
    }
}

pub async fn comfy(cli: &Cli, args: &ComfyArgs) -> Result<()> {
    match &args.command {
        ComfyCommand::Apps => {
            let rows = comfy_client(cli)?.apps().await?;
            if cli.json {
                emit(json!(rows), false)
            } else {
                output::table(
                    &rows,
                    &[("应用ID", "appId"), ("名称", "appName"), ("说明", "intro")],
                );
                Ok(())
            }
        }
        ComfyCommand::Instances { app_id } => {
            let rows = comfy_client(cli)?.instances(*app_id).await?;
            if cli.json {
                emit(json!(rows), false)
            } else {
                output::table(
                    &rows,
                    &[
                        ("实例ID", "instanceId"),
                        ("应用ID", "appId"),
                        ("区域ID", "areaId"),
                        ("Comfy状态(原值)", "instanceStatus"),
                    ],
                );
                Ok(())
            }
        }
        ComfyCommand::Run {
            app_id,
            instance_id,
            input,
            submit,
        } => {
            let input = generation::read_input(input)?;
            let body = json!({"appId":app_id.to_string(),"instanceId":instance_id.to_string(),"allowInput":input});
            if submit.dry_run {
                return preview(
                    cli,
                    &format!("{}/comfyuiTask/create", generation::COMFY_PREFIX),
                    body,
                );
            }
            confirm_task(cli, "Comfy", &body, submit.yes)?;
            task_accepted(
                comfy_client(cli)?
                    .run(*app_id, *instance_id, &input)
                    .await?,
            )
        }
        ComfyCommand::Status(args) => emit(
            comfy_client(cli)?.status(&args.task_id).await?,
            args.show_secrets,
        ),
        ComfyCommand::Upload {
            file,
            app_id,
            instance_id,
            dry_run,
        } => {
            let file = UploadFile::inspect(file, None)?;
            if *dry_run {
                return emit(
                    json!({"dryRun":true,"method":"POST","appId":app_id.to_string(),"instanceId":instance_id.to_string(),"origin":cli.origin(),"lookupPath":format!("{}/comfyuiInstance/list",generation::COMFY_PREFIX),"path":"<instance-host>/api/upload/image","multipart":{"image":{"name":file.name,"bytes":file.bytes},"type":"input"}}),
                    false,
                );
            }
            emit(
                comfy_client(cli)?
                    .upload(*app_id, *instance_id, &file)
                    .await?,
                false,
            )
        }
    }
}
