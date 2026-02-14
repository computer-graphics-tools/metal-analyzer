use std::sync::Arc;

use clap::Parser;
use metal_analyzer::server::MetalLanguageServer;
use tower_lsp::{Client, LspService, Server, lsp_types::MessageType};
use tracing::info;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(name = "metal-analyzer", version, about)]
struct Args {
    #[arg(long, short)]
    verbose: bool,

    #[arg(long)]
    log_messages: bool,

    #[arg(long)]
    log_file: Option<String>,
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
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            std::process::ExitCode::FAILURE
        },
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

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
