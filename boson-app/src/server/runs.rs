//! Run history server functions.
//!
//! `list_runs_datatable_page` reads one page at a time with the coordinator's keyset read
//! (`list_runs_after`) and a [`boson_core::RunFilter`] built from the request, so every filter
//! applies in the backend at any history size. The first page also returns
//! `count_runs_matching` as the total. The `show` rule puts runs of task types the reader's
//! teams own first ([`boson_backend::scan_parts`]); the cursor names the part as well as the
//! key, so paging keeps that order. A request past the first page whose cursor doesn't fit the
//! query walks forward to its offset by keyset, then hands out cursors again.
//! `list_runs_page` pages by offset.
//!
//! Access is per task type, as the session actor. The lists show every run but leave out the
//! error text of task types the reader can't View; a list filtered to one task type, and the
//! run page, need View on it.

use leptos::prelude::*;
use orbital_paging::{Page, PageRequest};

#[cfg(feature = "ssr")]
use super::helpers::{require_session, run_to_summary, trace_server_result};
#[cfg(feature = "ssr")]
use super::types::clamp_page_list_limit;
use super::types::{RunDetail, RunSummary};

/// Blank the error text on runs of task types the reader can't View (it can hold URLs,
/// account details or personal data).
#[cfg(feature = "ssr")]
async fn hide_run_errors(items: &mut [RunSummary]) {
    let mut access = super::access::ReaderAccess::from_context();
    for run in items {
        if !access.task_type(&run.task_name, "View").await {
            run.error_message = None;
        }
    }
}

/// Paginated runs endpoint.
#[uf_product_macros::server]
pub async fn list_runs_page(
    /// Zero-based index of the first run to return.
    offset: u32,
    /// Maximum number of runs to return.
    limit: u32,
    /// Optional job id to restrict results to runs of a single job.
    job_id_filter: Option<String>,
) -> Result<Page<RunSummary>, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        if let Some(ref job_id) = job_id_filter {
            boson_backend::validate_job_id(job_id)
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
        let limit = clamp_page_list_limit(limit);
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();

        let runs = backend
            .list_runs(
                job_id_filter.as_deref(),
                offset as usize,
                (limit + 1) as usize,
            )
            .await;

        let mut dtos: Vec<RunSummary> = runs.into_iter().map(run_to_summary).collect();
        hide_run_errors(&mut dtos).await;

        let total_count: Option<u64> = if offset == 0 {
            Some(backend.count_runs(job_id_filter.as_deref()).await)
        } else {
            None
        };

        Ok(Page::from_oversized(dtos, limit, total_count))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("list_runs_page", &result, None, None, None);
    result
}

/// Total for a runs list: every match, or under Only yours the matches of each owned task
/// type added up. `None` when the backend can't count.
#[cfg(feature = "ssr")]
async fn count_owned_runs(
    backend: &dyn boson_coordinator::BosonCoordinatorBackend,
    filter: &boson_core::RunFilter,
    mode: boson_backend::ShowMode,
    owned: &[String],
) -> Option<u64> {
    if mode != boson_backend::ShowMode::OnlyYours || owned.is_empty() {
        return backend.count_runs_matching(filter).await.ok();
    }
    let mut total = 0u64;
    for task in owned {
        if filter.task.as_ref().is_some_and(|t| t != task) {
            continue;
        }
        let mut one = filter.clone();
        one.task = Some(task.clone());
        total += backend.count_runs_matching(&one).await.ok()?;
    }
    Some(total)
}

/// Paginated runs for DataTable, filtered and paged in the backend by keyset cursor.
///
/// Filter rules (`status`, `task_name`, `from`, `to`) and the quick search become a
/// [`boson_core::RunFilter`] ([`boson_backend::run_filter_from_request`]). The search matches a
/// run id or its last characters; a whole job id instead lists that job's runs.
#[uf_product_macros::server]
pub async fn list_runs_datatable_page(
    /// `DataTable` paging/filter/search/sort request from the client.
    request: PageRequest,
    /// Optional job id to scope results to a single job's runs.
    scope_job_id: Option<String>,
) -> Result<Page<RunSummary>, ServerFnError> {
    let result = async {
        use boson_backend::{PartKey, PartStart};

        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let limit = clamp_page_list_limit(request.limit);
        let scope = scope_job_id.as_deref().filter(|s| !s.is_empty());
        let mut filter =
            boson_backend::run_filter_from_request(&request, scope, chrono::Utc::now());
        if let Some(ref job_id) = filter.job_id {
            boson_backend::validate_job_id(job_id)
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
        if crate::e2e_lab::list_load_fails(filter.task.as_deref()) {
            return Err(ServerFnError::new(
                "Failed to list runs: coordinator unavailable",
            ));
        }
        if let Some(task) = filter.task.as_deref() {
            super::access::require_task_type(task, "View").await?;
        }
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();

        // A whole job id in the search lists that job's runs.
        if let Some(text) = filter.id_match.clone() {
            if filter.job_id.is_none()
                && boson_backend::validate_job_id(&text).is_ok()
                && backend.get_job(&text).await.is_some()
            {
                filter.job_id = Some(text);
                filter.id_match = None;
            }
        }

        let mut ownership = super::access::Ownership::from_context();
        let owned = ownership
            .owned(crate::permissions::BOSON_TASK_TYPE_KIND)
            .await;
        let mode = boson_backend::ShowMode::from_request(&request);
        let is_owned = |r: &boson_core::Run| owned.contains(&r.task_name);
        let key_of = |r: &boson_core::Run| boson_core::RunKey::from(r);
        let read = |_part, after: Option<boson_core::RunKey>, n| {
            let filter = &filter;
            async move { backend.list_runs_after(filter, after.as_ref(), n).await }
        };
        let start = match boson_backend::runs_part_start(&request, scope, mode, !owned.is_empty()) {
            PartStart::At(at) => Some(at),
            // No cursor fits: walk forward to the offset, then page from there.
            PartStart::Offset(offset) => {
                let first = PartKey {
                    part: boson_backend::ScanPart::first(mode, !owned.is_empty()),
                    key: None,
                };
                boson_backend::scan_parts(first, mode, offset, is_owned, key_of, read)
                    .await
                    .resume
            }
        };
        let scan = match start {
            Some(at) => {
                boson_backend::scan_parts(at, mode, limit as usize, is_owned, key_of, read).await
            }
            None => boson_backend::PartScan {
                rows: Vec::new(),
                resume: None,
            },
        };
        // Count on the first page only; cursor pages report `None`.
        let total_count = if request.is_first_page() {
            count_owned_runs(backend, &filter, mode, &owned).await
        } else {
            None
        };
        let mut page =
            boson_backend::runs_page_from_scan(scan, &request, scope, limit, total_count);
        super::jobs::fill_max_attempts(backend, &mut page.items, |r| {
            (r.task_name.as_str(), &mut r.max_attempts)
        })
        .await;
        hide_run_errors(&mut page.items).await;
        for run in &mut page.items {
            run.yours = owned.contains(&run.task_name);
            run.owner = Some(ownership.task_type_owner(&run.task_name).await);
        }
        Ok(page)
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("list_runs_datatable_page", &result, None, None, None);
    result
}

/// Registered task names, sorted, for the Runs task filter.
#[uf_product_macros::server]
pub async fn list_task_names() -> Result<Vec<String>, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let mut names: Vec<String> = backend
            .registry()
            .iter()
            .map(|desc| desc.name.to_string())
            .collect();
        names.sort();
        Ok(names)
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("list_task_names", &result, None, None, None);
    result
}

/// Get a single run by id. Needs View on the run's task type.
#[uf_product_macros::server]
pub async fn get_run(
    /// Unique identifier of the run to look up.
    run_id: String,
) -> Result<Option<RunSummary>, ServerFnError> {
    let id_for_trace = run_id.clone();
    let result = async {
        boson_backend::validate_run_id(&run_id).map_err(|e| ServerFnError::new(e.to_string()))?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let Some(run) = backend.get_run(&run_id).await else {
            return Ok(None);
        };
        super::access::require_task_type(&run.task_name, "View").await?;
        Ok(Some(run_to_summary(run)))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_run", &result, None, None, Some(&id_for_trace));
    result
}

/// Most runs of one job the attempts list loads.
#[cfg(feature = "ssr")]
const MAX_ATTEMPTS_LISTED: usize = 100;

/// A run with its job, every attempt of that job and the task's attempt budget.
///
/// Needs View on the run's task type. An unknown id returns `None` before the check.
#[uf_product_macros::server]
pub async fn get_run_detail(
    /// Unique identifier of the run to look up.
    run_id: String,
) -> Result<Option<RunDetail>, ServerFnError> {
    let id_for_trace = run_id.clone();
    let result = async {
        boson_backend::validate_run_id(&run_id).map_err(|e| ServerFnError::new(e.to_string()))?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let Some(run) = backend.get_run(&run_id).await else {
            return Ok(None);
        };
        super::access::require_task_type(&run.task_name, "View").await?;
        let job = backend
            .get_job(&run.job_id)
            .await
            .map(|j| boson_backend::run_job_info(&j, super::helpers::person_name));
        let max_attempts = backend
            .get_task_config(&run.task_name)
            .await
            .map_err(|e| ServerFnError::new(boson_backend::format_task_config_load_error(e)))?
            .retry_policy
            .max_attempts
            .max(1);
        let mut attempts = backend
            .list_runs_after(
                &boson_core::RunFilter::job(&run.job_id),
                None,
                MAX_ATTEMPTS_LISTED,
            )
            .await;
        attempts.sort_by(|a, b| {
            a.attempt
                .cmp(&b.attempt)
                .then(a.started_at.cmp(&b.started_at))
        });
        let can_run = super::access::ReaderAccess::from_context()
            .task_type(&run.task_name, "Run")
            .await;
        let running_for_ms = (run.status == boson_core::RunStatus::Running).then(|| {
            (chrono::Utc::now() - run.started_at)
                .num_milliseconds()
                .max(0)
        });
        Ok(Some(RunDetail {
            run: run_to_summary(run),
            job,
            max_attempts,
            attempts: attempts.into_iter().map(run_to_summary).collect(),
            running_for_ms,
            can_run,
        }))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_run_detail", &result, None, None, Some(&id_for_trace));
    result
}
