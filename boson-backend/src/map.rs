//! DTO mappers and pure aggregates backing Boson job/run/task server fns.

use std::collections::HashMap;
use std::fmt::Write as _;

use boson_core::{
    IdempotencyMode, Job, JobStatus, RateLimitPolicy, RetryPolicy, Run, RunStatus, TaskConfig,
};
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::types::{
    DashboardStats, IdempotencyModeDto, JobStatusDto, JobSummary, RateLimitPolicyDto,
    RetryPolicyDto, RunJobInfo, RunStatusDto, RunSummary, TaskArgumentRow, TaskConfigDto,
    TaskSummary, UpdateTaskConfigRequest,
};

/// Per-task aggregates built from full job/run lists (`get_tasks`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaskStatsAgg {
    /// Count of jobs in [`JobStatus::Queued`] for the task.
    pub jobs_queued: u32,
    /// Total runs for the task.
    pub runs_total: u32,
    /// Runs with [`RunStatus::Success`].
    pub success_count: u32,
}

/// Maps a core job status onto the UI wire enum.
#[must_use]
pub const fn job_status_to_dto(s: JobStatus) -> JobStatusDto {
    match s {
        JobStatus::Queued => JobStatusDto::Queued,
        JobStatus::Running => JobStatusDto::Running,
        JobStatus::Success => JobStatusDto::Success,
        JobStatus::Failed => JobStatusDto::Failed,
        JobStatus::Canceled => JobStatusDto::Canceled,
    }
}

/// Maps a core run status onto the UI wire enum.
#[must_use]
pub const fn run_status_to_dto(s: RunStatus) -> RunStatusDto {
    match s {
        RunStatus::Running => RunStatusDto::Running,
        RunStatus::Success => RunStatusDto::Success,
        RunStatus::Failed => RunStatusDto::Failed,
        RunStatus::Canceled => RunStatusDto::Canceled,
        RunStatus::Timeout => RunStatusDto::Timeout,
    }
}

/// Maps a core retry policy onto the UI DTO.
#[must_use]
pub const fn retry_policy_to_dto(r: &RetryPolicy) -> RetryPolicyDto {
    RetryPolicyDto {
        max_attempts: r.max_attempts,
        base_delay_ms: r.base_delay_ms,
        backoff_multiplier: r.backoff_multiplier,
        max_delay_ms: r.max_delay_ms,
    }
}

/// Maps a UI retry policy DTO onto the core type.
#[must_use]
pub const fn retry_policy_from_dto(r: &RetryPolicyDto) -> RetryPolicy {
    RetryPolicy {
        max_attempts: r.max_attempts,
        base_delay_ms: r.base_delay_ms,
        backoff_multiplier: r.backoff_multiplier,
        max_delay_ms: r.max_delay_ms,
    }
}

/// Maps core enqueue limits onto the UI DTO.
#[must_use]
pub const fn rate_limit_policy_to_dto(r: &RateLimitPolicy) -> RateLimitPolicyDto {
    RateLimitPolicyDto {
        max_in_flight: r.max_in_flight,
        max_enqueue_per_second: r.max_enqueue_per_second,
    }
}

/// Maps UI enqueue limits onto the core type.
#[must_use]
pub const fn rate_limit_policy_from_dto(r: &RateLimitPolicyDto) -> RateLimitPolicy {
    RateLimitPolicy {
        max_in_flight: r.max_in_flight,
        max_enqueue_per_second: r.max_enqueue_per_second,
    }
}

/// Maps the stored idempotency override onto the UI choice.
///
/// No override inherits Boson's default, which checks the key ([`IdempotencyMode::Lwt`]).
#[must_use]
pub const fn idempotency_mode_to_dto(mode: Option<IdempotencyMode>) -> IdempotencyModeDto {
    match mode {
        Some(IdempotencyMode::None) => IdempotencyModeDto::AtLeastOnce,
        Some(IdempotencyMode::Lwt) | None => IdempotencyModeDto::ExactlyOnce,
    }
}

/// Maps the UI choice onto the stored idempotency override.
#[must_use]
pub const fn idempotency_mode_from_dto(mode: IdempotencyModeDto) -> IdempotencyMode {
    match mode {
        IdempotencyModeDto::ExactlyOnce => IdempotencyMode::Lwt,
        IdempotencyModeDto::AtLeastOnce => IdempotencyMode::None,
    }
}

/// Parses lowercase job-status filter strings used by queue list endpoints.
///
/// Unknown / empty / wrong-case values yield [`None`] (no filter).
#[must_use]
pub fn parse_job_status_filter(s: &str) -> Option<JobStatus> {
    match s {
        "queued" => Some(JobStatus::Queued),
        "running" => Some(JobStatus::Running),
        "success" => Some(JobStatus::Success),
        "failed" => Some(JobStatus::Failed),
        "canceled" => Some(JobStatus::Canceled),
        _ => None,
    }
}

/// Maps a core [`Job`] onto a list-row [`JobSummary`].
#[must_use]
pub fn job_to_summary(j: Job) -> JobSummary {
    let waited_ms = (Utc::now() - j.created_at).num_milliseconds().max(0);
    JobSummary {
        args_summary: args_summary(&j.params_json),
        job_id: j.job_id,
        task_name: j.task_name,
        status: job_status_to_dto(j.status),
        priority: j.priority,
        pool: j.pool,
        created_at: j.created_at.to_rfc3339(),
        waited_ms,
        attempt: j.attempt,
        max_attempts: None,
        last_error: None,
        failed_at: None,
    }
}

/// Longest param value [`args_summary`] shows before cutting it with "…".
pub const ARGS_SUMMARY_VALUE_CHARS: usize = 24;

/// Short label for a job's params: the first two as `key value`, joined with " · ".
///
/// Strings longer than [`ARGS_SUMMARY_VALUE_CHARS`] end in "…", objects read `{…}` and arrays
/// `[n]`. Params that aren't an object show as one shortened value; an empty object is "".
///
/// ```
/// use boson_backend::args_summary;
/// use serde_json::json;
///
/// assert_eq!(args_summary(&json!({"asset": 7781, "sizes": [1, 2, 3]})), "asset 7781 · sizes [3]");
/// assert_eq!(args_summary(&json!({})), "");
/// ```
#[must_use]
pub fn args_summary(params: &serde_json::Value) -> String {
    match params {
        serde_json::Value::Object(map) => map
            .iter()
            .take(2)
            .map(|(k, v)| format!("{k} {}", short_value(v)))
            .collect::<Vec<_>>()
            .join(" · "),
        serde_json::Value::Null => String::new(),
        other => short_value(other),
    }
}

fn short_value(v: &serde_json::Value) -> String {
    let text = match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(_) => return "{…}".to_string(),
        serde_json::Value::Array(items) => return format!("[{}]", items.len()),
        other => other.to_string(),
    };
    if text.chars().count() > ARGS_SUMMARY_VALUE_CHARS {
        let cut: String = text.chars().take(ARGS_SUMMARY_VALUE_CHARS - 1).collect();
        format!("{cut}…")
    } else {
        text
    }
}

/// Maps a core [`Run`] onto a list-row [`RunSummary`].
#[must_use]
pub fn run_to_summary(r: Run) -> RunSummary {
    RunSummary {
        run_id: r.run_id,
        job_id: r.job_id,
        task_name: r.task_name,
        status: run_status_to_dto(r.status),
        attempt: r.attempt,
        started_at: r.started_at.to_rfc3339(),
        finished_at: r.finished_at.map(|dt| dt.to_rfc3339()),
        duration_ms: r.duration_ms,
        error_message: r.error_message,
        max_attempts: None,
    }
}

/// Success rate percent when there is at least one run; otherwise [`None`].
#[must_use]
pub fn success_rate_pct(runs_total: u32, success_count: u32) -> Option<f64> {
    if runs_total > 0 {
        Some((f64::from(success_count) / f64::from(runs_total)) * 100.0)
    } else {
        None
    }
}

/// Builds a [`TaskSummary`] from descriptor fields, config, and aggregates.
#[must_use]
pub fn task_summary_from_parts(
    name: &str,
    signature_json: &str,
    default_priority: i32,
    default_pool: &str,
    config: &TaskConfig,
    stats: TaskStatsAgg,
) -> TaskSummary {
    TaskSummary {
        name: name.to_string(),
        signature_json: signature_json.to_string(),
        default_priority,
        default_pool: default_pool.to_string(),
        effective_priority: config.priority,
        effective_pool: config.pool.clone(),
        jobs_queued: stats.jobs_queued,
        runs_total: stats.runs_total,
        success_rate_pct: success_rate_pct(stats.runs_total, stats.success_count),
        owner: None,
    }
}

/// Aggregates per-task queued-job and run-success counters.
#[must_use]
pub fn aggregate_task_stats(jobs: &[Job], runs: &[Run]) -> HashMap<String, TaskStatsAgg> {
    let mut out: HashMap<String, TaskStatsAgg> = HashMap::new();

    for job in jobs {
        let entry = out.entry(job.task_name.clone()).or_default();
        if job.status == JobStatus::Queued {
            entry.jobs_queued += 1;
        }
    }

    for run in runs {
        let entry = out.entry(run.task_name.clone()).or_default();
        entry.runs_total += 1;
        if run.status == RunStatus::Success {
            entry.success_count += 1;
        }
    }

    out
}

/// Maps a core [`TaskConfig`] onto [`TaskConfigDto`].
#[must_use]
pub fn task_config_to_dto(config: &TaskConfig) -> TaskConfigDto {
    TaskConfigDto {
        task_name: config.task_name.clone(),
        priority: config.priority,
        pool: config.pool.clone(),
        retry_policy: retry_policy_to_dto(&config.retry_policy),
        rate_limit_policy: rate_limit_policy_to_dto(&config.rate_limit_policy),
        idempotency_mode: idempotency_mode_to_dto(config.idempotency_mode),
        updated_at: config.updated_at.to_rfc3339(),
    }
}

/// Applies a partial UI update onto a mutable [`TaskConfig`].
pub fn apply_task_config_update(
    config: &mut TaskConfig,
    req: &UpdateTaskConfigRequest,
    now: DateTime<Utc>,
) {
    if let Some(p) = req.priority {
        config.priority = p;
    }
    if let Some(ref p) = req.pool {
        config.pool.clone_from(p);
    }
    if let Some(ref r) = req.retry_policy {
        config.retry_policy = retry_policy_from_dto(r);
    }
    if let Some(ref r) = req.rate_limit_policy {
        config.rate_limit_policy = rate_limit_policy_from_dto(r);
    }
    if let Some(mode) = req.idempotency_mode {
        config.idempotency_mode = Some(idempotency_mode_from_dto(mode));
    }
    config.updated_at = now;
}

/// Last four characters of an id behind an ellipsis ("…0412"); short ids come back whole.
#[must_use]
pub fn short_id(id: &str) -> String {
    let count = id.chars().count();
    if count <= 4 {
        return id.to_string();
    }
    let tail: String = id.chars().skip(count - 4).collect();
    format!("…{tail}")
}

/// Who enqueued a job, from its `actor_json`.
///
/// `System` actors read "System", service users "<service> app", and users go through
/// `lookup` (the host's people lookup), falling back to the user id. Anything else reads
/// "Unknown".
#[must_use]
pub fn actor_label(actor_json: &Value, lookup: impl Fn(&str) -> Option<String>) -> String {
    if actor_json.get("System").is_some() {
        return "System".to_string();
    }
    if let Some(service) = actor_json
        .get("ServiceUser")
        .and_then(|s| s.get("service_name"))
        .and_then(Value::as_str)
    {
        let name = service.strip_prefix("svc:").unwrap_or(service);
        return format!("{name} app");
    }
    if let Some(user_id) = actor_json
        .get("User")
        .and_then(|u| u.get("user_id"))
        .and_then(Value::as_str)
    {
        return lookup(user_id).unwrap_or_else(|| user_id.to_string());
    }
    "Unknown".to_string()
}

/// Longest argument value [`job_label`] shows next to the id.
const JOB_LABEL_ARG_MAX_CHARS: usize = 40;

/// Short job label: the id's tail, then the first short text argument ("…0087 · batch b-42").
#[must_use]
pub fn job_label(job_id: &str, params_json: &Value) -> String {
    let short = short_id(job_id);
    let arg = params_json.as_object().and_then(|map| {
        map.iter().find_map(|(name, value)| {
            value
                .as_str()
                .filter(|v| !v.is_empty() && v.chars().count() <= JOB_LABEL_ARG_MAX_CHARS)
                .map(|v| format!("{name} {v}"))
        })
    });
    arg.map_or_else(|| short.clone(), |a| format!("{short} · {a}"))
}

/// Builds the run page's job block from a core [`Job`].
#[must_use]
pub fn run_job_info(job: &Job, lookup: impl Fn(&str) -> Option<String>) -> RunJobInfo {
    RunJobInfo {
        job_id: job.job_id.clone(),
        label: job_label(&job.job_id, &job.params_json),
        status: job_status_to_dto(job.status),
        pool: job.pool.clone(),
        started_by: actor_label(&job.actor_json, lookup),
    }
}

/// Run duration for display: "12 ms", "30.0 s" or "2 min 5 s".
#[must_use]
pub fn format_duration_ms(ms: i64) -> String {
    if ms < 1_000 {
        return format!("{ms} ms");
    }
    if ms < 60_000 {
        #[allow(clippy::cast_precision_loss)]
        let secs = ms as f64 / 1_000.0;
        return format!("{secs:.1} s");
    }
    let total_secs = ms / 1_000;
    let (mins, secs) = (total_secs / 60, total_secs % 60);
    if secs == 0 {
        format!("{mins} min")
    } else {
        format!("{mins} min {secs} s")
    }
}

/// A wait for retry sentences: "1 s", "1.5 s", "5 min", "1 h".
#[must_use]
pub fn format_wait_ms(ms: u64) -> String {
    if ms < 60_000 {
        if ms.is_multiple_of(1_000) {
            return format!("{} s", ms / 1_000);
        }
        #[allow(clippy::cast_precision_loss)]
        let secs = ms as f64 / 1_000.0;
        return format!("{secs:.1} s");
    }
    if ms < 3_600_000 {
        let mins = ms / 60_000;
        let rest = (ms % 60_000) / 1_000;
        return if rest == 0 {
            format!("{mins} min")
        } else {
            format!("{mins} min {rest} s")
        };
    }
    let hours = ms / 3_600_000;
    let rest = (ms % 3_600_000) / 60_000;
    if rest == 0 {
        format!("{hours} h")
    } else {
        format!("{hours} h {rest} min")
    }
}

/// Wait before retry `n` (1-based), as the worker computes it:
/// `min(base × multiplier^(n - 1), max)`.
#[must_use]
pub fn retry_wait_ms(policy: &RetryPolicyDto, n: u32) -> u64 {
    let exp = i32::try_from(n.saturating_sub(1)).unwrap_or(i32::MAX);
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let raw = (policy.base_delay_ms as f64) * policy.backoff_multiplier.powi(exp);
    #[allow(clippy::cast_precision_loss)]
    let cap = policy.max_delay_ms as f64;
    if !raw.is_finite() || raw >= cap {
        return policy.max_delay_ms;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let wait = raw.max(0.0) as u64;
    wait.min(policy.max_delay_ms)
}

/// Most retries [`retry_schedule_sentence`] spells out before "and so on".
const RETRY_SENTENCE_STEPS: u32 = 3;

/// What a retry policy does, in one line:
/// "With these values: retry after 1 s, then 2 s. A job gives up after 3 attempts."
#[must_use]
pub fn retry_schedule_sentence(policy: &RetryPolicyDto) -> String {
    let attempts = policy.max_attempts.max(1);
    let gives_up = if attempts == 1 {
        "A job gives up after 1 attempt.".to_string()
    } else {
        format!("A job gives up after {attempts} attempts.")
    };
    let retries = attempts - 1;
    if retries == 0 {
        return format!("With these values: no retries. {gives_up}");
    }
    let shown = retries.min(RETRY_SENTENCE_STEPS);
    let waits: Vec<String> = (1..=shown)
        .map(|n| format_wait_ms(retry_wait_ms(policy, n)))
        .collect();
    let mut list = format!("retry after {}", waits[0]);
    for w in &waits[1..] {
        list.push_str(", then ");
        list.push_str(w);
    }
    if retries > shown {
        let _ = write!(
            list,
            ", and so on up to {}",
            format_wait_ms(policy.max_delay_ms)
        );
    }
    format!("With these values: {list}. {gives_up}")
}

/// The task's settings in one sentence, ending with when they last changed:
/// "Runs on global at priority 1, up to 3 attempts, waiting 1 s then doubling up to 5 min.
/// Changed on Nov 18."
#[must_use]
pub fn settings_sentence(config: &TaskConfig) -> String {
    let r = &config.retry_policy;
    let attempts = r.max_attempts.max(1);
    let mut out = format!(
        "Runs on {} at priority {}, up to {attempts} attempt{}",
        config.pool,
        config.priority,
        if attempts == 1 { "" } else { "s" }
    );
    if attempts > 1 {
        let growth = if (r.backoff_multiplier - 2.0).abs() < f64::EPSILON {
            "doubling".to_string()
        } else if (r.backoff_multiplier - 1.0).abs() < f64::EPSILON {
            "the same each time".to_string()
        } else {
            format!("growing ×{}", r.backoff_multiplier)
        };
        let _ = write!(
            out,
            ", waiting {} then {growth} up to {}",
            format_wait_ms(r.base_delay_ms),
            format_wait_ms(r.max_delay_ms)
        );
    }
    out.push('.');
    let limit = config.rate_limit_policy.max_in_flight;
    if limit > 0 {
        let _ = write!(out, " At most {limit} waiting or running at once.");
    }
    let _ = write!(
        out,
        " Changed on {}.",
        config.updated_at.format("%b %-d, %Y")
    );
    out
}

/// Plain-language label for a JSON Schema `type`.
fn schema_type_label(ty: Option<&str>) -> &'static str {
    match ty {
        Some("string") => "Text",
        Some("integer") => "Whole number",
        Some("number") => "Number",
        Some("boolean") => "Yes or no",
        Some("array") => "List",
        Some("object") => "Object",
        Some("null") => "Nothing",
        _ => "Any",
    }
}

/// Arguments table rows from a task's signature JSON (a JSON Schema object).
///
/// Reads `properties` (name → `type`, `description`). A signature without `properties`, or
/// one that isn't JSON, has no rows.
#[must_use]
pub fn task_arguments_from_signature(signature_json: &str) -> Vec<TaskArgumentRow> {
    let Ok(value) = serde_json::from_str::<Value>(signature_json) else {
        return Vec::new();
    };
    let Some(props) = value.get("properties").and_then(Value::as_object) else {
        return Vec::new();
    };
    props
        .iter()
        .map(|(name, prop)| TaskArgumentRow {
            name: name.clone(),
            type_label: schema_type_label(prop.get("type").and_then(Value::as_str)).to_string(),
            description: prop
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
        .collect()
}

/// Top-level `description` from a task's signature JSON, if it has one.
#[must_use]
pub fn task_description_from_signature(signature_json: &str) -> Option<String> {
    serde_json::from_str::<Value>(signature_json)
        .ok()?
        .get("description")?
        .as_str()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(str::to_string)
}

/// Whether a task is at its "most at once" limit (`0` means no limit).
#[must_use]
pub const fn at_in_flight_limit(active: u32, max_in_flight: u32) -> bool {
    max_in_flight > 0 && active >= max_in_flight
}

/// Builds dashboard KPI counters after coordinator queries resolve.
#[must_use]
pub const fn dashboard_stats(
    task_count: u32,
    jobs_queued: u32,
    jobs_running: u32,
    runs_today: u32,
) -> DashboardStats {
    DashboardStats {
        task_count,
        jobs_queued,
        jobs_running,
        runs_today,
    }
}

/// Picker rows when the host provides no [`crate::BosonPoolProvider`]: `global` only.
#[must_use]
pub fn default_gluon_pool_rows() -> Vec<crate::types::GluonPoolPickRow> {
    vec![crate::types::GluonPoolPickRow {
        id: "global".to_string(),
        label: "global (default)".to_string(),
        detail: "Default in-process pool name when no Gluon pool is used.".to_string(),
    }]
}
