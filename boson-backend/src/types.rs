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
    /// Team or person that owns the task type, when the host knows one.
    ///
    /// Boson's registry carries no owner, so this stays `None` until the host supplies the
    /// owner from the task type's Access settings.
    #[serde(default)]
    pub owner: Option<String>,
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
    /// Why, in plain words ("it isn't a failed job"); coordinator internals stay on the server.
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

/// Enqueue limits exposed to the UI (`0` in either field means no limit).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitPolicyDto {
    /// Most jobs of the task waiting or running at once.
    pub max_in_flight: u32,
    /// Most enqueues per second, counted by each server on its own.
    pub max_enqueue_per_second: u32,
}

/// How enqueue treats a repeated idempotency key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyModeDto {
    /// Check the key so a job runs once (Boson's default).
    #[default]
    ExactlyOnce,
    /// Skip the check; faster, but a key can run twice.
    AtLeastOnce,
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
    /// Enqueue limits.
    pub rate_limit_policy: RateLimitPolicyDto,
    /// Duplicate handling for idempotency keys.
    pub idempotency_mode: IdempotencyModeDto,
    /// RFC3339 last-updated timestamp.
    pub updated_at: String,
}

/// One task argument, read from the task's signature JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskArgumentRow {
    /// Argument name.
    pub name: String,
    /// Plain-language type ("Text", "Whole number", …).
    pub type_label: String,
    /// Description from the signature, if it has one.
    pub description: Option<String>,
}

/// Everything the task type page shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskDetail {
    /// Registry summary (name, queue, totals, owner).
    pub summary: TaskSummary,
    /// One-line description from the signature JSON, if it has one.
    pub description: Option<String>,
    /// Jobs waiting to run.
    pub waiting: u32,
    /// Runs started in the last 24 hours.
    pub runs_24h: u32,
    /// Success rate of the runs started in the last 24 hours.
    pub success_rate_24h_pct: Option<f64>,
    /// The settings in one sentence, ending with when they last changed.
    pub settings_sentence: String,
    /// Arguments from the signature JSON.
    pub arguments: Vec<TaskArgumentRow>,
    /// Latest runs of this task, newest first.
    pub recent_runs: Vec<RunSummary>,
    /// Whether the session may open the Settings tab.
    pub can_edit: bool,
}

/// The job behind a run, as the run page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunJobInfo {
    /// Job id.
    pub job_id: String,
    /// Short label: the id's tail and the first short text argument.
    pub label: String,
    /// Current job status.
    pub status: JobStatusDto,
    /// Queue (pool) the job runs on.
    pub pool: String,
    /// Who enqueued it ("System", an app, or a person).
    pub started_by: String,
}

/// Everything the run page shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunDetail {
    /// The run itself.
    pub run: RunSummary,
    /// Its job, when the job is still stored.
    pub job: Option<RunJobInfo>,
    /// Total tries the task's retry policy allows.
    pub max_attempts: u32,
    /// Every run of the job, oldest first.
    pub attempts: Vec<RunSummary>,
    /// How long a running run has been going, measured when the page loaded.
    pub running_for_ms: Option<i64>,
    /// Whether the session may retry, drop or cancel the job.
    pub can_edit: bool,
}

/// Resolves a person's display name from a user id for "Started by".
///
/// Hosts with a people directory provide an `Arc<dyn BosonPeopleLookup>` in Leptos request
/// context; without one the run page shows the user id. Boson stores no names itself.
pub trait BosonPeopleLookup: Send + Sync {
    /// Display name for `user_id`, or `None` when the host doesn't know it.
    fn display_name(&self, user_id: &str) -> Option<String>;
}

/// Aggregate counters shown on the Boson dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardStats {
    /// Jobs waiting in every queue.
    pub jobs_waiting: u32,
    /// Jobs currently running.
    pub jobs_running: u32,
    /// Runs that failed in the last 24 hours.
    pub failed_24h: u32,
    /// `created_at` of the oldest waiting job across every queue, if any are waiting.
    pub oldest_waiting_at: Option<chrono::DateTime<chrono::Utc>>,
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateTaskConfigRequest {
    /// New priority when set.
    pub priority: Option<i32>,
    /// New pool when set.
    pub pool: Option<String>,
    /// New retry policy when set.
    pub retry_policy: Option<RetryPolicyDto>,
    /// New enqueue limits when set.
    #[serde(default)]
    pub rate_limit_policy: Option<RateLimitPolicyDto>,
    /// New duplicate handling when set.
    #[serde(default)]
    pub idempotency_mode: Option<IdempotencyModeDto>,
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
