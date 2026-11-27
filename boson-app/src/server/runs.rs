//! Run history server functions.
//!
//! `list_runs_datatable_page` reads one page at a time with the coordinator's keyset read
//! (`list_runs_after`) and a [`boson_core::RunFilter`] built from the request, so every filter
//! applies in the backend at any history size. The first page also returns
//! `count_runs_matching` as the total. A request past the first page whose cursor doesn't fit
//! the query walks forward to its offset by keyset, then hands out cursors again.
//! `list_runs_page` pages by offset.

use leptos::prelude::*;
use orbital_paging::{Page, PageRequest};

#[cfg(feature = "ssr")]
use super::helpers::{require_session, run_to_summary, trace_server_result};
use super::types::RunSummary;
#[cfg(feature = "ssr")]
use super::types::{clamp_page_list_limit, MAX_PAGE_LIST_LIMIT};

/// Paginated runs endpoint.
#[uf_product_macros::server(permission = "BosonAdmin")]
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

        let dtos: Vec<RunSummary> = runs.into_iter().map(run_to_summary).collect();

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

/// Key of the run just before `offset` in `filter`'s order, read forward by keyset.
///
/// Serves a page whose cursor doesn't fit the query; `None` when `offset` is 0.
#[cfg(feature = "ssr")]
async fn run_key_at_offset(
    backend: &dyn boson_coordinator::BosonCoordinatorBackend,
    filter: &boson_core::RunFilter,
    offset: usize,
) -> Option<boson_core::RunKey> {
    let mut key: Option<boson_core::RunKey> = None;
    let mut left = offset;
    while left > 0 {
        let chunk = left.min(MAX_PAGE_LIST_LIMIT as usize);
        let rows = backend.list_runs_after(filter, key.as_ref(), chunk).await;
        let Some(last) = rows.last() else { break };
        key = Some(boson_core::RunKey::from(last));
        left -= rows.len();
        if rows.len() < chunk {
            break;
        }
    }
    key
}

/// Paginated runs for DataTable, filtered and paged in the backend by keyset cursor.
///
/// Filter rules (`status`, `task_name`, `from`, `to`) and the quick search become a
/// [`boson_core::RunFilter`] ([`boson_backend::run_filter_from_request`]). The search matches a
/// run id or its last characters; a whole job id instead lists that job's runs.
#[uf_product_macros::server(permission = "BosonAdmin")]
pub async fn list_runs_datatable_page(
    /// `DataTable` paging/filter/search/sort request from the client.
    request: PageRequest,
    /// Optional job id to scope results to a single job's runs.
    scope_job_id: Option<String>,
) -> Result<Page<RunSummary>, ServerFnError> {
    let result = async {
        use boson_backend::ListStart;

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

        let key = match boson_backend::runs_list_start(&request, scope) {
            ListStart::After(key) => key,
            ListStart::Offset(offset) => run_key_at_offset(backend, &filter, offset).await,
        };
        let rows = backend
            .list_runs_after(&filter, key.as_ref(), (limit + 1) as usize)
            .await;
        // Count on the first page only; cursor pages report `None`.
        let total_count = if request.is_first_page() {
            backend.count_runs_matching(&filter).await.ok()
        } else {
            None
        };
        let mut page = boson_backend::runs_page_from_rows(
            rows,
            &request,
            scope,
            limit,
            &ListStart::After(key),
            total_count,
        );
        super::jobs::fill_max_attempts(backend, &mut page.items, |r| {
            (r.task_name.as_str(), &mut r.max_attempts)
        })
        .await;
        Ok(page)
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("list_runs_datatable_page", &result, None, None, None);
    result
}

/// Registered task names, sorted, for the Runs task filter.
#[uf_product_macros::server(permission = "BosonAdmin")]
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

/// Get a single run by id.
#[uf_product_macros::server(permission = "BosonAdmin")]
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
        Ok(backend.get_run(&run_id).await.map(run_to_summary))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_run", &result, None, None, Some(&id_for_trace));
    result
}
