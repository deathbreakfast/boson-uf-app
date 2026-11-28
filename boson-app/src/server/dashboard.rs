//! Dashboard statistics, Needs attention, Failing most and run trend server functions.

use leptos::prelude::*;

use super::types::{DashboardChartSeries, DashboardStats, FailingTaskRow, NeedsAttentionLine};

#[cfg(feature = "ssr")]
use super::helpers::{require_session, trace_server_result};
#[cfg(feature = "ssr")]
use boson_core::{JobStatus, RunFilter, RunStatus};

/// Failed runs read per task type to find its most common error.
#[cfg(feature = "ssr")]
const ERROR_SAMPLE_CAP: usize = 200;

/// Pool stats over the last hour, or an empty list when this host's coordinator is remote
/// (the queue lines and the oldest-waiting card then stay empty).
#[cfg(feature = "ssr")]
async fn pool_stats_or_empty(
    backend: &dyn boson_coordinator::BosonCoordinatorBackend,
) -> Result<Vec<boson_core::PoolStats>, ServerFnError> {
    let Ok(queue) = super::helpers::queue_backend(backend) else {
        return Ok(Vec::new());
    };
    queue
        .pool_stats(chrono::Duration::seconds(
            boson_backend::QUEUE_STATS_WINDOW_SECS,
        ))
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to load queue stats: {e}")))
}

#[cfg(feature = "ssr")]
async fn count_runs(
    backend: &dyn boson_coordinator::BosonCoordinatorBackend,
    filter: &RunFilter,
) -> Result<u64, ServerFnError> {
    backend
        .count_runs_matching(filter)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to count runs: {e}")))
}

#[cfg(feature = "ssr")]
fn runs_filter(
    task: Option<&str>,
    statuses: Vec<RunStatus>,
    from: chrono::DateTime<chrono::Utc>,
    to: Option<chrono::DateTime<chrono::Utc>>,
) -> RunFilter {
    RunFilter {
        statuses,
        task: task.map(str::to_string),
        started_from: Some(from),
        started_to: to,
        ..RunFilter::default()
    }
}

/// Run outcomes for every registered task type over the last 24 hours.
#[cfg(feature = "ssr")]
async fn load_task_failure_stats(
    backend: &dyn boson_coordinator::BosonCoordinatorBackend,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<boson_backend::TaskFailureStats>, ServerFnError> {
    let day_ago = now - chrono::Duration::hours(24);
    let week_before = day_ago - chrono::Duration::days(7);
    let mut out = Vec::with_capacity(backend.registry().len());
    for desc in backend.registry().iter() {
        let task = Some(desc.name);
        let runs_24h = count_runs(backend, &runs_filter(task, vec![], day_ago, None)).await?;
        let succeeded_24h = count_runs(
            backend,
            &runs_filter(task, vec![RunStatus::Success], day_ago, None),
        )
        .await?;
        let failed_filter = runs_filter(task, vec![RunStatus::Failed], day_ago, None);
        let failed_24h = count_runs(backend, &failed_filter).await?;
        let failed_prior_7d = count_runs(
            backend,
            &runs_filter(task, vec![RunStatus::Failed], week_before, Some(day_ago)),
        )
        .await?;
        let most_common_error = if failed_24h > 0 {
            let runs = backend
                .list_runs_after(&failed_filter, None, ERROR_SAMPLE_CAP)
                .await;
            boson_backend::most_common_error(runs.iter().filter_map(|r| r.error_message.as_deref()))
        } else {
            None
        };
        out.push(boson_backend::TaskFailureStats {
            task: desc.name.to_string(),
            runs_24h,
            succeeded_24h,
            failed_24h,
            failed_prior_7d,
            most_common_error,
        });
    }
    Ok(out)
}

/// Dashboard KPIs: jobs waiting, running, failed runs in the last 24 hours, oldest waiting.
#[uf_product_macros::server]
pub async fn get_dashboard_stats() -> Result<DashboardStats, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();

        let jobs_waiting =
            u32::try_from(backend.count_jobs(Some(JobStatus::Queued)).await).unwrap_or(u32::MAX);
        let jobs_running =
            u32::try_from(backend.count_jobs(Some(JobStatus::Running)).await).unwrap_or(u32::MAX);
        let day_ago = chrono::Utc::now() - chrono::Duration::hours(24);
        let failed_24h = count_runs(
            backend,
            &runs_filter(None, vec![RunStatus::Failed], day_ago, None),
        )
        .await?;
        let pools = pool_stats_or_empty(backend).await?;

        Ok(boson_backend::dashboard_stats(
            jobs_waiting,
            jobs_running,
            u32::try_from(failed_24h).unwrap_or(u32::MAX),
            boson_backend::oldest_waiting_across(&pools),
        ))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_dashboard_stats", &result, None, None, None);
    result
}

/// Needs attention lines, worst first: queues falling behind, failed jobs per task type, and
/// task types failing more than usual. Empty means All clear.
///
/// Requires `BosonAdmin`: the lines summarize every queue and task type at once, so this
/// app-wide panel stays with the app's admins. Per-resource numbers live on each page.
#[uf_product_macros::server(permission = "BosonAdmin")]
pub async fn get_needs_attention() -> Result<Vec<NeedsAttentionLine>, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let now = chrono::Utc::now();

        let pools = pool_stats_or_empty(backend).await?;
        let mut failed_jobs = Vec::new();
        for desc in backend.registry().iter() {
            let filter = boson_core::JobFilter {
                statuses: vec![JobStatus::Failed],
                task: Some(desc.name.to_string()),
                ..boson_core::JobFilter::default()
            };
            let count = backend
                .count_jobs_matching(&filter)
                .await
                .map_err(|e| ServerFnError::new(format!("Failed to count jobs: {e}")))?;
            failed_jobs.push(boson_backend::FailedJobsByTask {
                task: desc.name.to_string(),
                count,
            });
        }
        let tasks = load_task_failure_stats(backend, now).await?;
        let mut lines = boson_backend::needs_attention_lines(&pools, &failed_jobs, &tasks, now);
        let mut ownership = super::access::Ownership::from_context();
        let owned_tasks = ownership
            .owned(crate::permissions::BOSON_TASK_TYPE_KIND)
            .await;
        let owned_queues = ownership.owned(crate::permissions::BOSON_QUEUE_KIND).await;
        for line in &mut lines {
            if let Some(task) = line.task.clone() {
                line.yours = owned_tasks.contains(&task);
                line.owner = ownership.task_type_owner(&task).await;
            } else if let Some(queue) = line.queue.clone() {
                line.yours = owned_queues.contains(&queue);
                line.owner = ownership.queue_owner(&queue).await;
            }
        }
        // The panel splits into Your teams and Other teams; keep each part worst first.
        let (mut mine, rest): (Vec<_>, Vec<_>) = lines.into_iter().partition(|l| l.yours);
        mine.extend(rest);
        Ok(mine)
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_needs_attention", &result, None, None, None);
    result
}

/// Failing most: task types with the most failed runs in the last 24 hours.
///
/// Requires `BosonAdmin` (an app-wide panel), and lists only task types the reader can View:
/// rows carry each type's most common run error, which can hold free-form text.
#[uf_product_macros::server(permission = "BosonAdmin")]
pub async fn get_failing_most() -> Result<Vec<FailingTaskRow>, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let tasks = load_task_failure_stats(backend.as_ref(), chrono::Utc::now()).await?;
        let mut access = super::access::ReaderAccess::from_context();
        let mut ownership = super::access::Ownership::from_context();
        let owned = ownership
            .owned(crate::permissions::BOSON_TASK_TYPE_KIND)
            .await;
        let mut rows = Vec::new();
        for mut row in boson_backend::failing_most_rows(&tasks) {
            if access.task_type(&row.task, "View").await {
                row.yours = owned.contains(&row.task);
                row.owner = ownership.task_type_owner(&row.task).await;
                rows.push(row);
            }
        }
        Ok(boson_backend::order_owned(
            rows,
            &owned,
            boson_backend::ShowMode::YoursFirst,
            |r| r.task.as_str(),
        ))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_failing_most", &result, None, None, None);
    result
}

/// Run outcome counts per bucket for the dashboard chart.
///
/// Buckets are 4 hours wide for the 24h range and a day wide for 7d, in UTC. Each bucket
/// has three counts: succeeded, failed, and timed out or canceled. Empty buckets are kept so
/// the axis spans the whole window.
///
/// `range_secs` must be [`boson_backend::RANGE_SECS_24H`] or [`boson_backend::RANGE_SECS_7D`].
#[uf_product_macros::server]
pub async fn get_run_stats_series(
    /// Width of the trailing time window, in seconds, to aggregate run outcomes over.
    range_secs: i64,
) -> Result<Vec<DashboardChartSeries>, ServerFnError> {
    let result = async {
        boson_backend::validate_range_secs(range_secs)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();

        let now = chrono::Utc::now();
        let since = now - chrono::Duration::seconds(range_secs);
        let step =
            boson_backend::run_bucket_step(boson_backend::run_bucket_granularity(range_secs));
        let starts = boson_backend::run_bucket_starts(now, range_secs);
        let mut counts = Vec::with_capacity(starts.len());
        for start in &starts {
            let from = (*start).max(since);
            let to = *start + step;
            let mut bucket = [0u32; 3];
            for (slot, group) in bucket.iter_mut().zip(boson_backend::RunOutcomeGroup::ALL) {
                let n = count_runs(
                    backend,
                    &runs_filter(None, group.statuses(), from, Some(to)),
                )
                .await?;
                *slot = u32::try_from(n).unwrap_or(u32::MAX);
            }
            counts.push(bucket);
        }
        Ok(boson_backend::run_series_from_counts(&starts, &counts))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_run_stats_series", &result, None, None, None);
    result
}
