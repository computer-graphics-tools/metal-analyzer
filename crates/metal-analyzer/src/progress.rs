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

use std::{
    panic::AssertUnwindSafe,
    sync::atomic::{AtomicU64, Ordering},
};

use futures::FutureExt;
use tower_lsp::{Client, lsp_types::*};
use tracing::{debug, warn};

static NEXT_PROGRESS_ID: AtomicU64 = AtomicU64::new(1);
const PROGRESS_TITLE_PREFIX: &str = "metal-analyzer:";

/// A handle to an active work-done progress session.
///
/// Created by [`ProgressToken::begin`], which sends the
/// `window/workDoneProgress/create` request followed by a `$/progress`
/// `Begin` notification.  Call [`report`](Self::report) for intermediate
/// updates, and [`end`](Self::end) when the operation completes.
///
/// If dropped without calling `end`, the `Drop` impl sends a fire-and-forget
/// `End` notification so the editor never shows a stuck progress indicator.
pub struct ProgressToken {
    client: Option<Client>,
    token: Option<NumberOrString>,
}

impl ProgressToken {
    /// Start a new progress session.
    ///
    /// 1. Sends `window/workDoneProgress/create` to the client.
    /// 2. Sends `$/progress` with [`WorkDoneProgressBegin`].
    ///
    /// If the create request fails (e.g. the editor doesn't support it),
    /// the begin notification is still sent — most editors tolerate this.
    pub async fn begin(
        client: &Client,
        title: &str,
        message: Option<String>,
    ) -> Self {
        let id = NEXT_PROGRESS_ID.fetch_add(1, Ordering::Relaxed);
        let token = NumberOrString::String(format!("metalAnalyzer/{title}/{id}"));
        let display_title = prefixed_progress_title(title);

        // Send workDoneProgress/create as a background task so that:
        // 1. We don't block if the editor is slow to respond.
        // 2. The oneshot receiver stays alive until the response arrives,
        //    avoiding a panic in tower-lsp's Pending::insert when the
        //    receiver is dropped before the response.
        let create_client = client.clone();
        let create_token = token.clone();
        tokio::spawn(async move {
            let result = AssertUnwindSafe(create_client.send_request::<request::WorkDoneProgressCreate>(
                WorkDoneProgressCreateParams {
                    token: create_token,
                },
            ))
            .catch_unwind()
            .await;
            match result {
                Ok(Ok(())) => {},
                Ok(Err(error)) => {
                    debug!("workDoneProgress/create failed (editor may not support it): {error}");
                },
                Err(_) => {
                    warn!("workDoneProgress/create panicked (client may have disconnected)");
                },
            }
        });

        let send_ok = AssertUnwindSafe(client.send_notification::<notification::Progress>(ProgressParams {
            token: token.clone(),
            value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(WorkDoneProgressBegin {
                title: display_title.clone(),
                cancellable: Some(false),
                message,
                percentage: None,
            })),
        }))
        .catch_unwind()
        .await;

        if send_ok.is_err() {
            warn!("progress begin notification panicked (client may have disconnected)");
            return Self {
                client: None,
                token: None,
            };
        }

        debug!("progress begin: {display_title}");

        Self {
            client: Some(client.clone()),
            token: Some(token),
        }
    }

    /// Send an intermediate progress update.
    ///
    /// `percentage` should be in `0..=100`.
    #[allow(dead_code)]
    pub async fn report(
        &self,
        message: Option<String>,
        percentage: Option<u32>,
    ) {
        let percentage = percentage.map(|p| p.min(100));

        if let (Some(client), Some(token)) = (&self.client, &self.token) {
            let _ = AssertUnwindSafe(client.send_notification::<notification::Progress>(ProgressParams {
                token: token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(WorkDoneProgressReport {
                    cancellable: Some(false),
                    message,
                    percentage,
                })),
            }))
            .catch_unwind()
            .await;
        }
    }

    /// Finish the progress session.
    ///
    /// Consumes `self` so that no further updates can be sent.
    pub async fn end(
        mut self,
        message: Option<String>,
    ) {
        let Some(client) = self.client.take() else {
            return;
        };
        let Some(token) = self.token.take() else {
            return;
        };

        debug!("progress end: {token:?}");

        let _ = AssertUnwindSafe(client.send_notification::<notification::Progress>(ProgressParams {
            token,
            value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                message,
            })),
        }))
        .catch_unwind()
        .await;
    }
}

impl Drop for ProgressToken {
    fn drop(&mut self) {
        if let (Some(client), Some(token)) = (self.client.take(), self.token.take()) {
            debug!("progress cancelled (drop): {token:?}");
            tokio::spawn(async move {
                let _ = AssertUnwindSafe(client.send_notification::<notification::Progress>(ProgressParams {
                    token,
                    value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                        message: Some("Cancelled".to_string()),
                    })),
                }))
                .catch_unwind()
                .await;
            });
        }
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
