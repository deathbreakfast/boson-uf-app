//! Job server functions: the Jobs tables, Cancel, and Retry / Drop for failed jobs.
//!
//! `list_jobs_datatable_page` reads one page at a time with the coordinator's keyset read
//! (`list_jobs_after`), passing a [`boson_core::JobFilter`] built from the tab, the request's
//! filter rules and its quick search ([`boson_backend::job_filter_from_request`]). The first
//! page also returns `count_jobs_matching` as the total. A request past the first page whose
//! cursor doesn't fit the query walks forward to its offset by keyset, then hands out cursors
//! again. `list_jobs_page` (infinite scroll) pages by offset.
//!
//! [`authorize_job_action`] is the one check behind Cancel, Retry and Drop: Run on the job's
//! task type, as the session actor. Rows also carry whether the reader holds Run, and the
//! arguments and errors of task types the reader can't View are left out.

use leptos::prelude::*;
use orbital_paging::{Page, PageRequest};

#[cfg(feature = "ssr")]
use super::helpers::{
    job_to_summary, parse_job_status_filter, require_session, trace_server_result,
};
#[cfg(feature = "ssr")]
use super::types::{clamp_page_list_limit, JobActionFailure, RetriedJob, MAX_PAGE_LIST_LIMIT};
use super::types::{JobActionOutcome, JobSummary};

/// What a person is doing to a job. [`authorize_job_action`] decides whether they may.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobAction {
    /// Stop a waiting or running job.
    Cancel,
    /// Start a failed job again as a new job.
    Retry,
    /// Take a failed job off the failed list for good.
    Drop,
}

/// The access check for every job action: Run on the job's task type, as the session actor.
///
/// Cancel, Retry and Drop all need Run; the action is passed so the one check stays the place
/// to tell them apart if they ever need different actions.
///
/// # Errors
///
/// A `permission_denied::boson_task_type.{task}.Run` error when the reader lacks Run.
#[cfg(feature = "ssr")]
async fn authorize_job_action(task_name: &str, action: JobAction) -> Result<(), ServerFnError> {
    match action {
        JobAction::Cancel | JobAction::Retry | JobAction::Drop => {
            super::access::require_task_type(task_name, "Run").await
        }
    }
}

/// The task type of a stored job, for the per-job check.
#[cfg(feature = "ssr")]
async fn job_task_name(
    backend: &dyn boson_coordinator::BosonCoordinatorBackend,
    job_id: &str,
) -> Option<String> {
    backend.get_job(job_id).await.map(|j| j.task_name)
}

/// Mark the rows the reader may act on, and blank the arguments and last error on rows of
/// task types the reader can't View. Both can carry per-user data.
#[cfg(feature = "ssr")]
async fn apply_job_access(items: &mut [JobSummary]) {
    let mut access = super::access::ReaderAccess::from_context();
    for job in items {
        job.can_run = access.task_type(&job.task_name, "Run").await;
        if !access.task_type(&job.task_name, "View").await {
            job.args_summary.clear();
            job.last_error = None;
        }
    }
}

/// The failure row for a job the reader may not act on.
#[cfg(feature = "ssr")]
fn denied_failure(job_id: String, task_name: &str) -> JobActionFailure {
    JobActionFailure {
        job_id,
        error: format!("you need Run on the task type {task_name}"),
    }
}

/// A plain reason for a job a bulk action couldn't touch, without coordinator internals.
#[cfg(feature = "ssr")]
fn action_failure_reason(error: &boson_core::BosonError) -> String {
    use boson_core::BosonError;
    match error {
        BosonError::JobNotFound(_) => "it no longer exists".to_string(),
        BosonError::InvalidConfig(_) => "it isn't a failed job".to_string(),
        BosonError::RateLimited(_) => {
            "its task type is at its rate limit; try again soon".to_string()
        }
        _ => "Boson couldn't complete it; try again".to_string(),
    }
}

/// Cancel a job.
#[uf_product_macros::server]
pub async fn cancel_job(
    /// Unique identifier of the job to cancel.
    job_id: String,
) -> Result<(), ServerFnError> {
    let id_for_trace = job_id.clone();
    let result = async {
        boson_backend::validate_job_id(&job_id).map_err(|e| ServerFnError::new(e.to_string()))?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let task_name = job_task_name(backend, &job_id).await.ok_or_else(|| {
            ServerFnError::new(format!("Failed to cancel job: job not found: {job_id}"))
        })?;
        authorize_job_action(&task_name, JobAction::Cancel).await?;
        backend
            .cancel_job(&job_id)
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to cancel job: {e}")))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("cancel_job", &result, None, Some(&id_for_trace), None);
    result
}

#[cfg(feature = "ssr")]
fn validate_job_ids(job_ids: &[String]) -> Result<(), ServerFnError> {
    if job_ids.is_empty() {
        return Err(ServerFnError::new("Choose at least one job"));
    }
    if job_ids.len() > MAX_PAGE_LIST_LIMIT as usize {
        return Err(ServerFnError::new(format!(
            "Choose at most {MAX_PAGE_LIST_LIMIT} jobs at once"
        )));
    }
    for id in job_ids {
        boson_backend::validate_job_id(id).map_err(|e| ServerFnError::new(e.to_string()))?;
    }
    Ok(())
}

/// Start failed jobs again, each as a new queued job enqueued by the session actor.
///
/// Each new job starts at attempt 1 with the same arguments and the task type's current config,
/// and links back through `retried_from`. Jobs that can't be retried (not failed, unknown) land
/// in [`JobActionOutcome::failed`] while the rest go ahead.
#[uf_product_macros::server]
pub async fn retry_jobs(
    /// Ids of the failed jobs to retry (1 to 100).
    job_ids: Vec<String>,
) -> Result<JobActionOutcome, ServerFnError> {
    let result = async {
        validate_job_ids(&job_ids)?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let actor_json = serde_json::to_value(ctx.actor())
            .map_err(|e| ServerFnError::new(format!("Failed to read the session actor: {e}")))?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let mut outcome = JobActionOutcome::default();
        for from_job_id in job_ids {
            if let Some(task_name) = job_task_name(backend, &from_job_id).await {
                if authorize_job_action(&task_name, JobAction::Retry)
                    .await
                    .is_err()
                {
                    outcome.failed.push(denied_failure(from_job_id, &task_name));
                    continue;
                }
            }
            match backend.retry_job(&from_job_id, actor_json.clone()).await {
                Ok(job_id) => {
                    let pool = backend
                        .get_job(&job_id)
                        .await
                        .map_or_else(String::new, |j| j.pool);
                    outcome.retried.push(RetriedJob {
                        from_job_id,
                        job_id,
                        pool,
                    });
                }
                Err(e) => outcome.failed.push(JobActionFailure {
                    job_id: from_job_id,
                    error: action_failure_reason(&e),
                }),
            }
        }
        Ok(outcome)
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("retry_jobs", &result, None, None, None);
    result
}

/// Drop failed jobs: they leave the failed list and never run again. Their runs stay.
///
/// Jobs that can't be dropped (not failed, unknown) land in [`JobActionOutcome::failed`] while
/// the rest go ahead.
#[uf_product_macros::server]
pub async fn drop_jobs(
    /// Ids of the failed jobs to drop (1 to 100).
    job_ids: Vec<String>,
) -> Result<JobActionOutcome, ServerFnError> {
    let result = async {
        validate_job_ids(&job_ids)?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let mut outcome = JobActionOutcome::default();
        for job_id in job_ids {
            if let Some(task_name) = job_task_name(backend, &job_id).await {
                if authorize_job_action(&task_name, JobAction::Drop)
                    .await
                    .is_err()
                {
                    outcome.failed.push(denied_failure(job_id, &task_name));
                    continue;
                }
            }
            match backend.drop_failed_job(&job_id).await {
                Ok(()) => outcome.dropped.push(job_id),
                Err(e) => outcome.failed.push(JobActionFailure {
                    job_id,
                    error: action_failure_reason(&e),
                }),
            }
        }
        Ok(outcome)
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("drop_jobs", &result, None, None, None);
    result
}

/// Paginated jobs endpoint.
#[uf_product_macros::server]
pub async fn list_jobs_page(
    /// Zero-based index of the first job to return.
    offset: u32,
    /// Maximum number of jobs to return.
    limit: u32,
    /// Optional job status name to filter by (e.g. "queued", "running").
    status_filter: Option<String>,
) -> Result<Page<JobSummary>, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let limit = clamp_page_list_limit(limit);
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let status = status_filter.as_deref().and_then(parse_job_status_filter);

        let jobs = backend
            .list_jobs(status, offset as usize, (limit + 1) as usize)
            .await;

        let mut dtos: Vec<JobSummary> = jobs.into_iter().map(job_to_summary).collect();
        apply_job_access(&mut dtos).await;

        let total_count: Option<u64> = if offset == 0 {
            Some(backend.count_jobs(status).await)
        } else {
            None
        };

        Ok(Page::from_oversized(dtos, limit, total_count))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("list_jobs_page", &result, None, None, None);
    result
}

/// Key of the job just before `offset` in `filter`'s order, read forward by keyset.
///
/// Serves a page whose cursor doesn't fit the query; `None` when `offset` is 0.
#[cfg(feature = "ssr")]
async fn job_key_at_offset(
    backend: &dyn boson_coordinator::BosonCoordinatorBackend,
    filter: &boson_core::JobFilter,
    offset: usize,
) -> Option<boson_core::JobKey> {
    let mut key: Option<boson_core::JobKey> = None;
    let mut left = offset;
    while left > 0 {
        let chunk = left.min(MAX_PAGE_LIST_LIMIT as usize);
        let rows = backend.list_jobs_after(filter, key.as_ref(), chunk).await;
        let Some(last) = rows.last() else { break };
        key = Some(boson_core::JobKey::from(last));
        left -= rows.len();
        if rows.len() < chunk {
            break;
        }
    }
    key
}

/// Fill `max_attempts` from each task type's current retry policy, one lookup per task.
#[cfg(feature = "ssr")]
pub(super) async fn fill_max_attempts<T>(
    backend: &dyn boson_coordinator::BosonCoordinatorBackend,
    items: &mut [T],
    task_and_slot: impl Fn(&mut T) -> (&str, &mut Option<u32>),
) {
    let mut cache: std::collections::HashMap<String, Option<u32>> =
        std::collections::HashMap::new();
    for item in items.iter_mut() {
        let (task, slot) = task_and_slot(item);
        if !cache.contains_key(task) {
            let max = backend
                .get_task_config(task)
                .await
                .ok()
                .map(|c| c.retry_policy.max_attempts);
            cache.insert(task.to_string(), max);
        }
        *slot = cache.get(task).copied().flatten();
    }
}

/// Paginated jobs for one Jobs tab, with filter rules and quick search.
///
/// `tab` is `active` (the default), `failed` or `finished`; see [`boson_backend::JobsTab`].
/// Failed rows also carry their latest run's error and finish time.
#[uf_product_macros::server]
pub async fn list_jobs_datatable_page(
    /// `DataTable` paging/filter/search/sort request from the client.
    request: PageRequest,
    /// Jobs tab key; unknown or missing is `active`.
    tab: Option<String>,
) -> Result<Page<JobSummary>, ServerFnError> {
    let result = async {
        use boson_backend::{JobsTab, ListStart};

        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let limit = clamp_page_list_limit(request.limit);
        let tab = JobsTab::from_key(tab.as_deref().unwrap_or_default());
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();

        let Some(filter) =
            boson_backend::job_filter_from_request(&request, tab, chrono::Utc::now())
        else {
            return Ok(Page {
                items: Vec::new(),
                has_more: false,
                total_count: request.is_first_page().then_some(0),
                ..Page::default()
            });
        };

        let key = match boson_backend::jobs_list_start(&request) {
            ListStart::After(key) => key,
            ListStart::Offset(offset) => job_key_at_offset(backend, &filter, offset).await,
        };
        let rows = backend
            .list_jobs_after(&filter, key.as_ref(), (limit + 1) as usize)
            .await;
        // Count on the first page only; cursor pages report `None`.
        let total_count = if request.is_first_page() {
            backend.count_jobs_matching(&filter).await.ok()
        } else {
            None
        };
        let mut page = boson_backend::jobs_page_from_rows(
            rows,
            &request,
            limit,
            &ListStart::After(key),
            total_count,
        );

        fill_max_attempts(backend, &mut page.items, |j| {
            (j.task_name.as_str(), &mut j.max_attempts)
        })
        .await;
        if tab == JobsTab::Failed {
            for job in &mut page.items {
                let last = backend
                    .list_runs_after(&boson_core::RunFilter::job(&job.job_id), None, 1)
                    .await
                    .into_iter()
                    .next();
                if let Some(run) = last {
                    job.failed_at = Some(run.finished_at.unwrap_or(run.started_at).to_rfc3339());
                    job.last_error = run.error_message;
                }
            }
        }
        apply_job_access(&mut page.items).await;
        Ok(page)
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("list_jobs_datatable_page", &result, None, None, None);
    result
}
