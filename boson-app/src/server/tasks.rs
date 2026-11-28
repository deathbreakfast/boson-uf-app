//! Task list, detail, config, and paginated task endpoints.
//!
//! Tasks come from the in-process registry, so `get_tasks_page` and `get_tasks_datatable_page`
//! load every task summary and page by offset; they return no cursor.
//!
//! Access is per task type, as the session actor: the lists show every task type with the
//! reader's role and no numbers where the reader can't View; the detail needs View for its
//! numbers and runs, the settings need Edit.

use leptos::prelude::*;
use orbital_paging::{Page, PageRequest};

#[cfg(feature = "ssr")]
use super::helpers::{
    apply_task_config_update, build_task_summary, load_all_task_summaries, require_email_verified,
    require_session, task_config_to_dto, trace_server_result,
};
#[cfg(feature = "ssr")]
use super::types::clamp_page_list_limit;
use super::types::{TaskConfigDto, TaskDetail, TaskSummary, UpdateTaskConfigRequest};

/// Get all tasks with effective config and stats.
#[uf_product_macros::server]
pub async fn get_tasks() -> Result<Vec<TaskSummary>, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let mut tasks = load_all_task_summaries(backend.as_ref()).await?;
        super::access::ReaderAccess::from_context()
            .label_tasks(&mut tasks)
            .await;
        Ok(tasks)
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_tasks", &result, None, None, None);
    result
}

/// Get a single task by name (O(1) registry lookup + task-scoped stats).
#[uf_product_macros::server]
pub async fn get_task(
    /// Registry name of the task to look up.
    task_name: String,
) -> Result<Option<TaskSummary>, ServerFnError> {
    let name_for_trace = task_name.clone();
    let result = async {
        boson_backend::validate_task_name(&task_name)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let Some(desc) = backend.registry().get(&task_name) else {
            return Ok(None);
        };
        let mut task = build_task_summary(backend, desc).await?;
        super::access::ReaderAccess::from_context()
            .label_tasks(std::slice::from_mut(&mut task))
            .await;
        Ok(Some(task))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_task", &result, Some(&name_for_trace), None, None);
    result
}

/// Runs the task type page lists under Recent runs.
#[cfg(feature = "ssr")]
const RECENT_RUNS: usize = 10;

/// Everything the task type page shows. `None` when no task type has that name.
///
/// Without View on the task type the detail keeps what the code declares (name, queue, owner,
/// description, arguments) and leaves out the numbers and runs, with `can_view` false, so the
/// page can say what's missing and offer Request access.
#[uf_product_macros::server]
pub async fn get_task_detail(
    /// Registry name of the task to look up.
    task_name: String,
) -> Result<Option<TaskDetail>, ServerFnError> {
    let name_for_trace = task_name.clone();
    let result = async {
        boson_backend::validate_task_name(&task_name)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let Some(desc) = backend.registry().get(&task_name) else {
            return Ok(None);
        };
        let mut access = super::access::ReaderAccess::from_context();
        let can_view = access.task_type(&task_name, "View").await;
        let can_run = access.task_type(&task_name, "Run").await;
        let can_edit = access.task_type(&task_name, "Edit").await;
        let mut summary = build_task_summary(backend, desc).await?;
        summary.access = access.task_type_label(&task_name).await.to_string();
        let mut ownership = super::access::Ownership::from_context();
        summary.owner = Some(ownership.task_type_owner(&task_name).await);
        summary.yours = ownership
            .owned(crate::permissions::BOSON_TASK_TYPE_KIND)
            .await
            .contains(&task_name);
        if !can_view {
            super::access::redact_task(&mut summary);
            return Ok(Some(TaskDetail {
                description: boson_backend::task_description_from_signature(desc.signature_json),
                waiting: 0,
                runs_24h: 0,
                success_rate_24h_pct: None,
                settings_sentence: String::new(),
                arguments: boson_backend::task_arguments_from_signature(desc.signature_json),
                recent_runs: Vec::new(),
                can_view,
                can_run,
                can_edit,
                summary,
            }));
        }
        let config = backend
            .get_task_config(&task_name)
            .await
            .map_err(|e| ServerFnError::new(boson_backend::format_task_config_load_error(e)))?;

        // Runs come newest first; the counts use the coordinator's filtered count.
        let task_filter = boson_core::RunFilter {
            task: Some(task_name.clone()),
            ..boson_core::RunFilter::default()
        };
        let runs = backend
            .list_runs_after(&task_filter, None, RECENT_RUNS)
            .await;
        let day = boson_core::RunFilter {
            started_from: Some(chrono::Utc::now() - chrono::Duration::hours(24)),
            ..task_filter.clone()
        };
        let day_success = boson_core::RunFilter {
            statuses: vec![boson_core::RunStatus::Success],
            ..day.clone()
        };
        let count = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        let runs_24h = count(backend.count_runs_matching(&day).await.unwrap_or(0));
        let success_24h = count(backend.count_runs_matching(&day_success).await.unwrap_or(0));

        Ok(Some(TaskDetail {
            description: boson_backend::task_description_from_signature(desc.signature_json),
            waiting: summary.jobs_queued,
            runs_24h,
            success_rate_24h_pct: boson_backend::success_rate_pct(runs_24h, success_24h),
            settings_sentence: boson_backend::settings_sentence(&config),
            arguments: boson_backend::task_arguments_from_signature(desc.signature_json),
            // Handler error text can hold URLs, account details or personal data, so the
            // recent runs leave it out; the run page shows it.
            recent_runs: runs
                .into_iter()
                .map(|r| {
                    let mut summary = boson_backend::run_to_summary(r);
                    summary.error_message = None;
                    summary
                })
                .collect(),
            can_view,
            can_run,
            can_edit,
            summary,
        }))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result(
        "get_task_detail",
        &result,
        Some(&name_for_trace),
        None,
        None,
    );
    result
}

/// How many jobs of the task are waiting or running now (what "Most at once" counts).
///
/// Needs View on the task type.
#[uf_product_macros::server]
pub async fn get_task_active_jobs(
    /// Registry name of the task to count.
    task_name: String,
) -> Result<u32, ServerFnError> {
    let name_for_trace = task_name.clone();
    let result = async {
        use boson_core::JobStatus;

        boson_backend::validate_task_name(&task_name)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        super::access::require_task_type(&task_name, "View").await?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let queued = backend
            .count_jobs_for_task(&task_name, Some(JobStatus::Queued))
            .await;
        let running = backend
            .count_jobs_for_task(&task_name, Some(JobStatus::Running))
            .await;
        Ok(u32::try_from(queued.saturating_add(running)).unwrap_or(u32::MAX))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result(
        "get_task_active_jobs",
        &result,
        Some(&name_for_trace),
        None,
        None,
    );
    result
}

/// Get task config. Needs Edit on the task type (it feeds the Settings form).
#[uf_product_macros::server]
pub async fn get_task_config(
    /// Registry name of the task whose config should be fetched.
    task_name: String,
) -> Result<TaskConfigDto, ServerFnError> {
    let name_for_trace = task_name.clone();
    let result = async {
        boson_backend::validate_task_name(&task_name)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        require_email_verified().await?;
        super::access::require_task_type(&task_name, "Edit").await?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let config = backend
            .get_task_config(&task_name)
            .await
            .map_err(|e| ServerFnError::new(format!("Task config not found: {e}")))?;
        Ok(task_config_to_dto(&config))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result(
        "get_task_config",
        &result,
        Some(&name_for_trace),
        None,
        None,
    );
    result
}

/// Update task config. Needs Edit on the task type.
#[uf_product_macros::server]
pub async fn update_task_config(
    /// Registry name of the task whose config should be updated.
    task_name: String,
    /// Partial update request with the fields to change.
    req: UpdateTaskConfigRequest,
) -> Result<TaskConfigDto, ServerFnError> {
    let name_for_trace = task_name.clone();
    let result = async {
        boson_backend::validate_task_name(&task_name)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        require_email_verified().await?;
        super::access::require_task_type(&task_name, "Edit").await?;
        boson_backend::validate_task_config_update(&req)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let backend = super::helpers::boson_backend()?;
        let backend = backend.as_ref();
        let mut config = backend
            .get_task_config(&task_name)
            .await
            .map_err(|e| ServerFnError::new(format!("Task config not found: {e}")))?;
        boson_backend::validate_pool_offered(&req, &config.pool, &super::helpers::offered_pools())
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        apply_task_config_update(&mut config, &req, chrono::Utc::now());
        backend
            .upsert_task_config(config.clone())
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to update config: {e}")))?;

        Ok(task_config_to_dto(&config))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result(
        "update_task_config",
        &result,
        Some(&name_for_trace),
        None,
        None,
    );
    result
}

/// Paginated tasks endpoint.
///
/// Returns a [`Page<TaskSummary>`] using the standard `orbital-paging`
/// over-fetch pattern. Tasks come from the in-memory registry (typically
/// small) so we fetch all, sort, filter, then slice.
#[uf_product_macros::server]
pub async fn get_tasks_page(
    /// Zero-based index of the first task to return.
    offset: u32,
    /// Maximum number of tasks to return.
    limit: u32,
    /// Optional case-insensitive search string matched against name, signature, and pool.
    query: Option<String>,
) -> Result<Page<TaskSummary>, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let limit = clamp_page_list_limit(limit);
        let backend = super::helpers::boson_backend()?;
        let mut tasks = load_all_task_summaries(backend.as_ref()).await?;
        boson_backend::filter_tasks_by_query(&mut tasks, query.as_deref());
        super::access::ReaderAccess::from_context()
            .label_tasks(&mut tasks)
            .await;

        let total_count: Option<u64> = if offset == 0 {
            Some(tasks.len() as u64)
        } else {
            None
        };

        let sliced: Vec<TaskSummary> = tasks
            .into_iter()
            .skip(offset as usize)
            .take((limit + 1) as usize)
            .collect();

        Ok(Page::from_oversized(sliced, limit, total_count))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_tasks_page", &result, None, None, None);
    result
}

/// Paginated tasks for DataTable toolbar (quick search via PageRequest).
#[uf_product_macros::server]
pub async fn get_tasks_datatable_page(
    /// `DataTable` paging/filter/search/sort request from the client.
    request: PageRequest,
) -> Result<Page<TaskSummary>, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let limit = clamp_page_list_limit(request.limit);
        let backend = super::helpers::boson_backend()?;
        let mut tasks = load_all_task_summaries(backend.as_ref()).await?;
        boson_backend::filter_tasks_by_query(
            &mut tasks,
            super::page_query::quick_search_text(&request).as_deref(),
        );
        super::access::ReaderAccess::from_context()
            .label_tasks(&mut tasks)
            .await;
        let mut ownership = super::access::Ownership::from_context();
        let owned = ownership
            .owned(crate::permissions::BOSON_TASK_TYPE_KIND)
            .await;
        for task in &mut tasks {
            task.yours = owned.contains(&task.name);
            task.owner = Some(ownership.task_type_owner(&task.name).await);
        }
        let tasks = boson_backend::order_owned(
            tasks,
            &owned,
            boson_backend::ShowMode::from_request(&request),
            |t| t.name.as_str(),
        );

        let total_count: Option<u64> = if request.offset == 0 {
            Some(tasks.len() as u64)
        } else {
            None
        };

        let sliced: Vec<TaskSummary> = tasks
            .into_iter()
            .skip(request.offset as usize)
            .take((limit + 1) as usize)
            .collect();

        Ok(Page::from_oversized(sliced, limit, total_count))
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_tasks_datatable_page", &result, None, None, None);
    result
}
