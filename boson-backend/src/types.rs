//! UI-facing DTOs and paging constants for Boson server contracts.

use serde::{Deserialize, Serialize};

/// Wire status for a queued or active Boson job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatusDto {
    /// Waiting to be claimed by a worker.
    Queued,
    /// Currently executing.
    Running,
    /// Finished successfully.
    Success,
    /// Finished with failure.
    Failed,
    /// Cancelled before or during execution.
    Canceled,
}

/// Wire status for a single Boson run attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatusDto {
    /// Attempt currently in progress.
    Running,
    /// Attempt finished successfully.
    Success,
    /// Attempt finished with failure.
    Failed,
    /// Attempt cancelled.
    Canceled,
    /// Attempt exceeded its time budget.
    Timeout,
}

/// Summary of a registered task with effective config and aggregate stats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskSummary {
    /// Registry task name.
    pub name: String,
    /// JSON-encoded task signature / schema.
    pub signature_json: String,
    /// Default priority from the registry descriptor.
    pub default_priority: i32,
    /// Default pool from the registry descriptor.
    pub default_pool: String,
    /// Effective priority after config overlay.
    pub effective_priority: i32,
    /// Effective pool after config overlay.
    pub effective_pool: String,
    /// Count of jobs currently queued for this task.
    pub jobs_queued: u32,
    /// Total runs recorded for this task.
    pub runs_total: u32,
    /// Success rate percent when `runs_total > 0`.
    pub success_rate_pct: Option<f64>,
}

/// Summary of a Boson job for queue / list views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobSummary {
    /// Unique job identifier.
    pub job_id: String,
    /// Registry task name the job targets.
    pub task_name: String,
    /// Current job status.
    pub status: JobStatusDto,
    /// Scheduling priority.
    pub priority: i32,
    /// Execution pool name.
    pub pool: String,
    /// RFC3339 creation timestamp.
    pub created_at: String,
    /// Milliseconds from creation to when the row was read.
    pub waited_ms: i64,
    /// The first two params as `key value`, joined with " · " (see [`crate::args_summary`]).
    pub args_summary: String,
    /// Attempt number (1-based).
    pub attempt: i32,
    /// Most attempts the task type's current retry policy allows, when the server looked it up.
    pub max_attempts: Option<u32>,
    /// Error of the job's latest run (failed-jobs list only).
    pub last_error: Option<String>,
    /// RFC3339 finish time of the job's latest run (failed-jobs list only).
    pub failed_at: Option<String>,
}

/// Summary of a Boson run for history / detail views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    /// Unique run identifier.
    pub run_id: String,
    /// Parent job identifier.
    pub job_id: String,
    /// Registry task name.
    pub task_name: String,
    /// Run outcome status.
    pub status: RunStatusDto,
    /// Attempt number within the job.
    pub attempt: i32,
    /// RFC3339 start timestamp.
    pub started_at: String,
    /// RFC3339 finish timestamp, if finished.
    pub finished_at: Option<String>,
    /// Wall duration in milliseconds, if known.
    pub duration_ms: Option<i64>,
    /// Error message when the run failed.
    pub error_message: Option<String>,
    /// Most attempts the task type's current retry policy allows, when the server looked it up.
    pub max_attempts: Option<u32>,
}

/// A failed job that the `retry_jobs` server fn started again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetriedJob {
    /// The failed job that was retried.
    pub from_job_id: String,
    /// The new queued job.
    pub job_id: String,
    /// Pool the new job waits in.
    pub pool: String,
}

/// A job a bulk action couldn't act on, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobActionFailure {
    /// The job the action was for.
    pub job_id: String,
    /// What went wrong, in the coordinator's words.
    pub error: String,
}

/// What a bulk Retry or Drop did: each job lands in exactly one list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobActionOutcome {
    /// Jobs retried, with their new job ids (Retry only).
    pub retried: Vec<RetriedJob>,
    /// Ids of jobs dropped (Drop only).
    pub dropped: Vec<String>,
    /// Jobs the action failed for.
    pub failed: Vec<JobActionFailure>,
}

/// Retry policy fields exposed to the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicyDto {
    /// Maximum attempts including the first try.
    pub max_attempts: u32,
    /// Base backoff delay in milliseconds.
    pub base_delay_ms: u64,
    /// Multiplier applied between attempts.
    pub backoff_multiplier: f64,
    /// Cap on backoff delay in milliseconds.
    pub max_delay_ms: u64,
}

/// Task configuration DTO returned by get/update config endpoints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskConfigDto {
    /// Registry task name.
    pub task_name: String,
    /// Effective priority.
    pub priority: i32,
    /// Effective pool.
    pub pool: String,
    /// Effective retry policy.
    pub retry_policy: RetryPolicyDto,
    /// RFC3339 last-updated timestamp.
    pub updated_at: String,
}

/// Aggregate counters shown on the Boson dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardStats {
    /// Registered task count.
    pub task_count: u32,
    /// Jobs currently queued.
    pub jobs_queued: u32,
    /// Jobs currently running.
    pub jobs_running: u32,
    /// Runs started in the last 24 hours.
    pub runs_today: u32,
}

/// Time-series point for dashboard charts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardChartPoint {
    /// Bucket timestamp (UTC).
    pub ts: chrono::DateTime<chrono::Utc>,
    /// Aggregated value for the bucket.
    pub value: f64,
}

/// Named time series for dashboard charts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardChartSeries {
    /// Stable series id (e.g. `"successful"`).
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Ordered points spanning the selected range.
    pub points: Vec<DashboardChartPoint>,
}

/// Partial update request for task configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateTaskConfigRequest {
    /// New priority when set.
    pub priority: Option<i32>,
    /// New pool when set.
    pub pool: Option<String>,
    /// New retry policy when set.
    pub retry_policy: Option<RetryPolicyDto>,
}

/// Gluon virtual pool row for Boson task-config pool picker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GluonPoolPickRow {
    /// Virtual pool id (use as Boson `pool` string).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Secondary detail text.
    pub detail: String,
}

/// Pools a host offers in the task-config picker.
///
/// Hosts that place Boson workers in Pion pools provide an
/// `Arc<dyn BosonPoolProvider>` in Leptos request context; without one the
/// picker offers [`crate::default_gluon_pool_rows`]. `pools` runs on every
/// picker load and every pool change, so return a cached snapshot rather than
/// querying storage under the caller's session.
pub trait BosonPoolProvider: Send + Sync {
    /// Pools workers drain, `global` included when the host offers it.
    fn pools(&self) -> Vec<GluonPoolPickRow>;
}

/// Page size used by the tasks infinite scroll / `DataTable`.
pub const TASKS_PAGE_SIZE: u32 = 20;
/// Page size used by the jobs/queue infinite scroll / `DataTable`.
pub const JOBS_PAGE_SIZE: u32 = 20;
/// Page size used by the runs infinite scroll / `DataTable`.
pub const RUNS_PAGE_SIZE: u32 = 20;

/// Days a failed job stays on the failed list before the retention sweep drops it.
pub const FAILED_JOB_RETENTION_DAYS: i64 = boson_core::DEFAULT_FAILED_RETENTION.num_days();

/// Maximum rows returned by a single paginated list server fn.
pub const MAX_PAGE_LIST_LIMIT: u32 = 100;

/// Cap requested page list limits to [`MAX_PAGE_LIST_LIMIT`].
#[must_use]
pub fn clamp_page_list_limit(limit: u32) -> u32 {
    limit.min(MAX_PAGE_LIST_LIMIT)
}
