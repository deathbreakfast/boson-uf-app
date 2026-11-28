//! Queues (Gluon pools): list, detail, pause, resume and purge.
//!
//! Every endpoint goes through [`authorize_queue`], which today requires `BosonAdmin` for
//! each action. Per-queue access replaces that one helper.

use leptos::prelude::*;

use super::types::{QueueDetail, QueueSummary};

#[cfg(feature = "ssr")]
use super::helpers::{job_to_summary, require_session, trace_server_result};

/// What a caller wants to do with a queue; [`authorize_queue`] decides whether they may.
#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueAction {
    /// Read stats and jobs.
    View,
    /// Pause or resume claims.
    Edit,
    /// Remove waiting jobs.
    Purge,
}

/// The one queue access check. Every action needs `BosonAdmin` until per-queue access lands.
#[cfg(feature = "ssr")]
async fn authorize_queue(ctx: &higgs::Higgs, action: QueueAction) -> Result<(), ServerFnError> {
    require_session(ctx)?;
    match action {
        QueueAction::View | QueueAction::Edit | QueueAction::Purge => {
            uf_product::permissions::require_permission(super::BOSON_ADMIN_PERMISSION).await
        }
    }
}

#[cfg(feature = "ssr")]
fn validate_queue_name(name: &str) -> Result<String, ServerFnError> {
    boson_backend::validate_pool_name(name).map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(name.trim().to_string())
}

#[cfg(feature = "ssr")]
const fn window() -> chrono::Duration {
    chrono::Duration::seconds(boson_backend::QUEUE_STATS_WINDOW_SECS)
}

/// Summary row for one pool, or `None` when the backend doesn't know it.
#[cfg(feature = "ssr")]
async fn load_queue_summary(
    queue: &dyn boson_core::QueueBackend,
    name: &str,
) -> Result<Option<QueueSummary>, ServerFnError> {
    let stats = queue
        .pool_stats(window())
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to load queue stats: {e}")))?;
    let Some(row) = stats.into_iter().find(|p| p.pool == name) else {
        return Ok(None);
    };
    let pause = queue
        .get_pool_state(name)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to load queue state: {e}")))?;
    Ok(Some(boson_backend::queue_summary(&row, pause.as_ref())))
}

#[cfg(feature = "ssr")]
async fn require_queue_summary(
    queue: &dyn boson_core::QueueBackend,
    name: &str,
) -> Result<QueueSummary, ServerFnError> {
    load_queue_summary(queue, name)
        .await?
        .ok_or_else(|| ServerFnError::new(format!("Queue not found: {name}")))
}

/// Label stored with a pause: the signed-in user's display name or email, else their id.
#[cfg(feature = "ssr")]
async fn actor_label(ctx: &higgs::Higgs) -> String {
    if let Ok(user) = lepton_auth::extract_auth_user().await {
        if let Some(name) = user.display_name.filter(|n| !n.trim().is_empty()) {
            return name;
        }
        if !user.email.is_empty() {
            return user.email;
        }
    }
    ctx.session_user_id().cloned().unwrap_or_default()
}

/// Every queue with its state and numbers for the last hour, falling behind first.
#[uf_product_macros::server]
pub async fn list_queues() -> Result<Vec<QueueSummary>, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        authorize_queue(&ctx, QueueAction::View).await?;
        let backend = super::helpers::boson_backend()?;
        let queue = super::helpers::queue_backend(backend.as_ref())?;
        let stats = queue
            .pool_stats(window())
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to load queue stats: {e}")))?;
        let mut rows = Vec::with_capacity(stats.len());
        for row in &stats {
            let pause = if row.paused {
                queue
                    .get_pool_state(&row.pool)
                    .await
                    .map_err(|e| ServerFnError::new(format!("Failed to load queue state: {e}")))?
            } else {
                None
            };
            rows.push(boson_backend::queue_summary(row, pause.as_ref()));
        }
        boson_backend::sort_queues(&mut rows);
        Ok(rows)
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("list_queues", &result, None, None, None);
    result
}

/// One queue's numbers plus its waiting and running jobs.
#[uf_product_macros::server]
pub async fn get_queue(
    /// Pool name.
    name: String,
) -> Result<QueueDetail, ServerFnError> {
    let result = async {
        let name = validate_queue_name(&name)?;
        let ctx = higgs::Higgs::from_request().await?;
        authorize_queue(&ctx, QueueAction::View).await?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let queue = super::helpers::queue_backend(backend)?;
        let summary = require_queue_summary(queue.as_ref(), &name).await?;
        let filter = boson_core::JobFilter {
            statuses: vec![
                boson_core::JobStatus::Running,
                boson_core::JobStatus::Queued,
            ],
            pool: Some(name.clone()),
            ..boson_core::JobFilter::default()
        };
        let jobs = backend
            .list_jobs_after(&filter, None, boson_backend::QUEUE_DETAIL_JOBS_CAP)
            .await
            .into_iter()
            .map(job_to_summary)
            .collect();
        Ok(QueueDetail { summary, jobs })
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_queue", &result, None, None, None);
    result
}

#[cfg(feature = "ssr")]
async fn set_paused(name: &str, paused: bool) -> Result<QueueSummary, ServerFnError> {
    let name = validate_queue_name(name)?;
    let ctx = higgs::Higgs::from_request().await?;
    authorize_queue(&ctx, QueueAction::Edit).await?;
    let backend = super::helpers::boson_backend()?;
    let queue = super::helpers::queue_backend(backend.as_ref())?;
    require_queue_summary(queue.as_ref(), &name).await?;
    let label = actor_label(&ctx).await;
    queue
        .set_pool_paused(&name, paused, &label)
        .await
        .map_err(|e| {
            let verb = if paused { "pause" } else { "resume" };
            ServerFnError::new(format!("Failed to {verb} queue: {e}"))
        })?;
    require_queue_summary(queue.as_ref(), &name).await
}

/// Pause claims from a queue. Waiting jobs stay put, running jobs finish, and new jobs still
/// join the queue.
#[uf_product_macros::server]
pub async fn pause_queue(
    /// Pool name.
    name: String,
) -> Result<QueueSummary, ServerFnError> {
    let result = set_paused(&name, true).await;
    #[cfg(feature = "ssr")]
    trace_server_result("pause_queue", &result, None, None, None);
    result
}

/// Resume claims from a paused queue.
#[uf_product_macros::server]
pub async fn resume_queue(
    /// Pool name.
    name: String,
) -> Result<QueueSummary, ServerFnError> {
    let result = set_paused(&name, false).await;
    #[cfg(feature = "ssr")]
    trace_server_result("resume_queue", &result, None, None, None);
    result
}

/// Remove every waiting job from a queue and return how many went. Running jobs finish;
/// whatever enqueued the removed jobs isn't told.
#[uf_product_macros::server]
pub async fn purge_queue(
    /// Pool name.
    name: String,
) -> Result<u64, ServerFnError> {
    let result = async {
        let name = validate_queue_name(&name)?;
        let ctx = higgs::Higgs::from_request().await?;
        authorize_queue(&ctx, QueueAction::Purge).await?;
        let backend = super::helpers::boson_backend()?;
        let queue = super::helpers::queue_backend(backend.as_ref())?;
        require_queue_summary(queue.as_ref(), &name).await?;
        queue
            .purge_queued_in_pool(&name)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to purge queue: {e}")))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("purge_queue", &result, None, None, None);
    result
}
