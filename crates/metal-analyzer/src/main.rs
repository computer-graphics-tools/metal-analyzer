use clap::Parser;
use tower_lsp::{LspService, Server};
use tracing::info;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use metal_analyzer::MetalLanguageServer;

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
    let dir = dirs_or_tmp();
    dir.join("metal-analyzer.log")
}

fn dirs_or_tmp() -> std::path::PathBuf {
    if let Some(cache) = std::env::var_os("HOME") {
        let dir = std::path::PathBuf::from(cache).join(".metal-analyzer");
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    std::env::temp_dir()
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let stderr_filter = if args.verbose {
        EnvFilter::new("metal_analyzer=debug,tower_lsp=debug")
    } else {
        EnvFilter::new("metal_analyzer=info,tower_lsp=warn")
    };

    let file_filter = if args.verbose {
        EnvFilter::new("metal_analyzer=debug,tower_lsp=info")
    } else {
        // Keep baseline lifecycle logs without the heavy debug stream by default.
        EnvFilter::new("metal_analyzer=info,tower_lsp=warn")
    };

    let log_path = args
        .log_file
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(default_log_path);

    let file_appender = tracing_appender::rolling::never(
        log_path.parent().unwrap_or(std::path::Path::new(".")),
        log_path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("metal-analyzer.log")),
    );

    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_target(false)
        .with_filter(file_filter);

    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .with_filter(stderr_filter);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .init();

    info!("Starting Metal Analyzer server v{}", env!("CARGO_PKG_VERSION"));
    info!("Log file: {}", log_path.display());

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) =
        LspService::new(|client| MetalLanguageServer::new(client, args.log_messages));

    Server::new(stdin, stdout, socket).serve(service).await;

    info!("Metal Analyzer server stopped");
}
