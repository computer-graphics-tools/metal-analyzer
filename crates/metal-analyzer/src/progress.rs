//! Work-done progress reporting via the LSP `$/progress` notification.
//!
//! Mirrors the pattern used by rust-analyzer: the server creates a progress
//! token, sends `Begin` / `Report` / `End` notifications, and the editor
//! renders them in its activity indicator (e.g. the spinning icon in Zed's
//! status bar).
//!
//! ## Usage
//!
//! ```ignore
//! let token = ProgressToken::begin(&client, "Indexing AST", None).await;
//! token.report(Some("parsing…".into()), Some(50)).await;
//! token.end(Some("done".into())).await;
//! ```

use tower_lsp::Client;
use tower_lsp::lsp_types::*;
use tracing::debug;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROGRESS_ID: AtomicU64 = AtomicU64::new(1);
const PROGRESS_TITLE_PREFIX: &str = "Metal Analyzer:";

/// A handle to an active work-done progress session.
///
/// Created by [`ProgressToken::begin`], which sends the
/// `window/workDoneProgress/create` request followed by a `$/progress`
/// `Begin` notification.  Call [`report`](Self::report) for intermediate
/// updates, and [`end`](Self::end) when the operation completes.
///
/// `end` consumes `self` so that further updates on a finished token are
/// a compile-time error.
pub struct ProgressToken {
    client: Client,
    token: NumberOrString,
}

impl ProgressToken {
    /// Start a new progress session.
    ///
    /// 1. Sends `window/workDoneProgress/create` to the client.
    /// 2. Sends `$/progress` with [`WorkDoneProgressBegin`].
    ///
    /// If the create request fails (e.g. the editor doesn't support it),
    /// the begin notification is still sent — most editors tolerate this.
    pub async fn begin(client: &Client, title: &str, message: Option<String>) -> Self {
        let id = NEXT_PROGRESS_ID.fetch_add(1, Ordering::Relaxed);
        let token = NumberOrString::String(format!("metalAnalyzer/{title}/{id}"));
        let display_title = prefixed_progress_title(title);

        let create_result = client
            .send_request::<request::WorkDoneProgressCreate>(WorkDoneProgressCreateParams {
                token: token.clone(),
            })
            .await;

        if let Err(e) = create_result {
            debug!("workDoneProgress/create failed (editor may not support it): {e}");
        }

        client
            .send_notification::<notification::Progress>(ProgressParams {
                token: token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(
                    WorkDoneProgressBegin {
                        title: display_title.clone(),
                        cancellable: Some(false),
                        message,
                        percentage: None,
                    },
                )),
            })
            .await;

        debug!("progress begin: {display_title}");

        Self {
            client: client.clone(),
            token,
        }
    }

    /// Send an intermediate progress update.
    ///
    /// `percentage` should be in `0..=100`.
    #[allow(dead_code)]
    pub async fn report(&self, message: Option<String>, percentage: Option<u32>) {
        let percentage = percentage.map(|p| p.min(100));

        self.client
            .send_notification::<notification::Progress>(ProgressParams {
                token: self.token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
                    WorkDoneProgressReport {
                        cancellable: Some(false),
                        message,
                        percentage,
                    },
                )),
            })
            .await;
    }

    /// Finish the progress session.
    ///
    /// Consumes `self` so that no further updates can be sent.
    pub async fn end(self, message: Option<String>) {
        debug!("progress end: {:?}", self.token);

        self.client
            .send_notification::<notification::Progress>(ProgressParams {
                token: self.token,
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                    message,
                })),
            })
            .await;
    }
}

fn prefixed_progress_title(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.starts_with(PROGRESS_TITLE_PREFIX) {
        return trimmed.to_owned();
    }
    format!("{PROGRESS_TITLE_PREFIX} {trimmed}")
}

/// Fire-and-forget helper: report a short-lived operation that begins and
/// ends almost instantly (e.g. "Running Metal compiler…" → "3 diagnostics").
///
/// This is a convenience wrapper around [`ProgressToken`] for operations
/// where intermediate `report` calls are not needed.
#[allow(dead_code)]
pub async fn run_with_progress<F, T>(
    client: &Client,
    title: &str,
    begin_message: Option<String>,
    work: F,
) -> T
where
    F: std::future::Future<Output = (T, Option<String>)>,
{
    let token = ProgressToken::begin(client, title, begin_message).await;
    let (result, end_message) = work.await;
    token.end(end_message).await;
    result
}

#[cfg(test)]
#[path = "../tests/src/progress_tests.rs"]
mod tests;
