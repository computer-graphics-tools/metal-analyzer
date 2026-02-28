use std::sync::Arc;

use clap::Parser;
use metal_analyzer::server::{
    MetalLanguageServer,
    formatting::{FormattingError, clang_format_args, run_clang_format},
};
use tower_lsp::{Client, LspService, Server, lsp_types::MessageType};
use tracing::info;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "metal-analyzer", version, about)]
struct Args {
    #[arg(long, short, global = true)]
    verbose: bool,

    #[arg(long, global = true)]
    log_messages: bool,

    #[arg(long, global = true)]
    log_file: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Format Metal source files
    Format(FormatArgs),
}

#[derive(clap::Args, Debug)]
struct FormatArgs {
    /// Files to format. Use `-` or omit for stdin.
    files: Vec<String>,

    /// Check formatting without modifying files (exit 1 if changes needed)
    #[arg(long)]
    check: bool,

    /// Formatting command
    #[arg(long, default_value = "clang-format")]
    command: String,

    /// Extra arguments for the formatter
    #[arg(long)]
    args: Vec<String>,
}

fn default_log_path() -> std::path::PathBuf {
    let directory = log_directory();
    directory.join("metal-analyzer.log")
}

fn log_directory() -> std::path::PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let directory = std::path::PathBuf::from(home).join(".metal-analyzer");
        if std::fs::create_dir_all(&directory).is_ok() {
            return directory;
        }
    }
    std::env::temp_dir()
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::ExitCode::FAILURE
        },
    }
}

async fn run() -> Result<std::process::ExitCode, Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    match args.command {
        Some(Command::Format(fmt_args)) => run_format(fmt_args).await,
        None => {
            run_server(args).await?;
            Ok(std::process::ExitCode::SUCCESS)
        },
    }
}

async fn run_format(fmt_args: FormatArgs) -> Result<std::process::ExitCode, Box<dyn std::error::Error + Send + Sync>> {
    let use_stdin = fmt_args.files.is_empty() || (fmt_args.files.len() == 1 && fmt_args.files[0] == "-");

    if use_stdin {
        let mut input = String::new();
        tokio::io::AsyncReadExt::read_to_string(&mut tokio::io::stdin(), &mut input).await?;
        // When reading from stdin we don't have a real file path for config discovery.
        let args = clang_format_args(&fmt_args.args, "shader.metal".to_string());
        let formatted = run_clang_format_with_fallback(&fmt_args.command, &args, &input).await?;

        if fmt_args.check {
            if formatted != input {
                return Ok(std::process::ExitCode::from(1));
            }
            return Ok(std::process::ExitCode::SUCCESS);
        }

        print!("{formatted}");
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let mut has_diff = false;
    let mut has_error = false;

    for file_path in &fmt_args.files {
        let path = std::path::Path::new(file_path);
        let input = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) => {
                eprintln!("error: {file_path}: {error}");
                has_error = true;
                continue;
            },
        };

        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let args = clang_format_args(&fmt_args.args, canonical.display().to_string());

        let formatted = match run_clang_format_with_fallback(&fmt_args.command, &args, &input).await {
            Ok(output) => output,
            Err(error) => {
                eprintln!("error: {file_path}: {error}");
                has_error = true;
                continue;
            },
        };

        if formatted == input {
            continue;
        }

        if fmt_args.check {
            eprintln!("{file_path}");
            has_diff = true;
            continue;
        }

        if let Err(error) = std::fs::write(path, &formatted) {
            eprintln!("error: {file_path}: {error}");
            has_error = true;
        }
    }

    if has_error {
        Ok(std::process::ExitCode::FAILURE)
    } else if has_diff {
        Ok(std::process::ExitCode::from(1))
    } else {
        Ok(std::process::ExitCode::SUCCESS)
    }
}

async fn run_clang_format_with_fallback(
    command: &str,
    args: &[String],
    input: &str,
) -> Result<String, FormattingError> {
    match run_clang_format(command, args, input).await {
        Ok(output) => Ok(output),
        Err(FormattingError::CommandNotFound(_)) if command == "clang-format" => {
            let mut xcrun_args = vec!["clang-format".to_string()];
            xcrun_args.extend(args.iter().cloned());
            run_clang_format("xcrun", &xcrun_args, input).await
        },
        Err(error) => Err(error),
    }
}

async fn run_server(args: Args) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let stderr_filter = if args.verbose {
        EnvFilter::new("metal_analyzer=debug")
    } else {
        EnvFilter::new("metal_analyzer=info")
    };

    let file_filter = if args.verbose {
        EnvFilter::new("metal_analyzer=debug")
    } else {
        EnvFilter::new("metal_analyzer=info")
    };

    let log_path = args.log_file.as_ref().map(std::path::PathBuf::from).unwrap_or_else(default_log_path);

    let file_appender = tracing_appender::rolling::never(
        log_path.parent().unwrap_or(std::path::Path::new(".")),
        log_path.file_name().unwrap_or(std::ffi::OsStr::new("metal-analyzer.log")),
    );

    let file_layer =
        fmt::layer().with_writer(file_appender).with_ansi(false).with_target(false).with_filter(file_filter);

    let stderr_layer =
        fmt::layer().with_writer(std::io::stderr).with_ansi(false).with_target(false).with_filter(stderr_filter);

    tracing_subscriber::registry().with(file_layer).with(stderr_layer).init();

    info!("Starting metal-analyzer server v{}", env!("CARGO_PKG_VERSION"));
    info!("Log file: {}", log_path.display());

    let log_messages = args.log_messages;
    let (service, socket) = LspService::new(|client| {
        install_panic_hook(client.clone());
        MetalLanguageServer::new(client, log_messages)
    });

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    Server::new(stdin, stdout, socket).serve(service).await;

    info!("metal-analyzer server stopped");
    Ok(())
}

fn install_panic_hook(client: Client) {
    let previous_hook = std::panic::take_hook();
    let client = Arc::new(client);
    let weak_client = Arc::downgrade(&client);

    std::panic::set_hook(Box::new(move |panic_info| {
        tracing::error!("Server panicked: {panic_info}");

        if let Some(client) = weak_client.upgrade() {
            let message = format!(
                "metal-analyzer: encountered an internal error and may need to be restarted. \
                 Details: {panic_info}"
            );
            let _ = futures::executor::block_on(client.show_message(MessageType::ERROR, message));
        }

        previous_hook(panic_info);
    }));

    // Keep the Arc alive for the lifetime of the server.
    std::mem::forget(client);
}
