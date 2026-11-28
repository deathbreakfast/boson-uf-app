//! `PageRequest` helpers for Boson `DataTable` server adapters.
//!
//! The jobs and runs tables send their toolbar state as filter rules on the [`PageRequest`], so
//! the cursor fingerprint covers it. [`run_filter_from_request`] and [`job_filter_from_request`]
//! turn those rules and the quick search into the coordinator's [`RunFilter`] and [`JobFilter`],
//! which the keyset reads (`list_runs_after`, `list_jobs_after`) and the matching counts apply
//! in the backend. Nothing here filters rows in memory.
//!
//! Rules the builders read (every other field is ignored, and rules always combine with AND):
//!
//! | Field | Operator | Value | Filter field |
//! |---|---|---|---|
//! | `status` | `equals` / `is` | status keys, comma-separated | `statuses` |
//! | `task_name` | `equals` / `is` | task name | `task` |
//! | `job_id` | `equals` / `is` | whole job id (runs only) | `job_id` |
//! | `pool` | `equals` / `is` | pool name (jobs only) | `pool` |
//! | [`FROM_FIELD`] / [`TO_FIELD`] | any | a [time token](parse_time_range) | start or creation range |

use boson_core::{JobFilter, JobStatus, RunFilter, RunStatus};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use orbital_data::DataValue;
use orbital_paging::{FilterRuleParam, PageRequest};

use crate::types::{JobStatusDto, RunStatusDto};

/// Filter-rule field holding the start of the time range.
pub const FROM_FIELD: &str = "from";
/// Filter-rule field holding the end of the time range.
pub const TO_FIELD: &str = "to";

/// Lowercase status key for a run wire status.
#[must_use]
pub const fn run_status_key(status: RunStatusDto) -> &'static str {
    match status {
        RunStatusDto::Running => "running",
        RunStatusDto::Success => "success",
        RunStatusDto::Failed => "failed",
        RunStatusDto::Canceled => "canceled",
        RunStatusDto::Timeout => "timeout",
    }
}

/// Lowercase status key for a job wire status.
#[must_use]
pub const fn job_status_key(status: JobStatusDto) -> &'static str {
    match status {
        JobStatusDto::Queued => "queued",
        JobStatusDto::Running => "running",
        JobStatusDto::Success => "success",
        JobStatusDto::Failed => "failed",
        JobStatusDto::Canceled => "canceled",
    }
}

fn parse_run_status(key: &str) -> Option<RunStatus> {
    match key {
        "running" => Some(RunStatus::Running),
        "success" => Some(RunStatus::Success),
        "failed" => Some(RunStatus::Failed),
        "canceled" => Some(RunStatus::Canceled),
        "timeout" => Some(RunStatus::Timeout),
        _ => None,
    }
}

/// Status filter for jobs (`status` column equals rule), lowercased.
#[must_use]
pub fn extract_status_filter(request: &PageRequest) -> Option<String> {
    equals_rules(request, "status")
        .next()
        .map(|v| v.to_lowercase())
}

/// Job id from URL scope or structured filter on `job_id`.
#[must_use]
pub fn resolve_job_filter(scope_job: Option<String>, request: &PageRequest) -> Option<String> {
    scope_job
        .filter(|s| !s.is_empty())
        .or_else(|| equals_rules(request, "job_id").next())
}

/// Trimmed quick-search text, or [`None`] when blank.
#[must_use]
pub fn quick_search_text(request: &PageRequest) -> Option<String> {
    request
        .quick_search
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Which jobs a tab on the Jobs page lists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum JobsTab {
    /// Waiting and running jobs.
    #[default]
    Active,
    /// Jobs that used every attempt and haven't been dropped.
    Failed,
    /// Succeeded and canceled jobs.
    Finished,
}

impl JobsTab {
    /// Tab for a `?tab=` value; anything unknown is [`JobsTab::Active`].
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "failed" => Self::Failed,
            "finished" => Self::Finished,
            _ => Self::Active,
        }
    }

    /// The `?tab=` value for this tab.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Failed => "failed",
            Self::Finished => "finished",
        }
    }

    /// Statuses the tab lists.
    #[must_use]
    pub fn statuses(self) -> Vec<JobStatus> {
        match self {
            Self::Active => vec![JobStatus::Queued, JobStatus::Running],
            Self::Failed => vec![JobStatus::Failed],
            Self::Finished => vec![JobStatus::Success, JobStatus::Canceled],
        }
    }
}

fn equals_rules<'a>(request: &'a PageRequest, field: &'a str) -> impl Iterator<Item = String> + 'a {
    request
        .filter
        .iter()
        .flat_map(|f| f.items.iter())
        .filter(move |rule| {
            rule.field == field && matches!(rule.operator.as_str(), "equals" | "is")
        })
        .map(rule_text)
        .filter(|v| !v.is_empty())
}

fn rule_text(rule: &FilterRuleParam) -> String {
    match &rule.value {
        DataValue::Text(s) => s.trim().to_string(),
        other => other.display_string().trim().to_string(),
    }
}

fn first_rule(request: &PageRequest, field: &str) -> Option<String> {
    request
        .filter
        .iter()
        .flat_map(|f| f.items.iter())
        .find(|rule| rule.field == field)
        .map(rule_text)
        .filter(|v| !v.is_empty())
}

/// Status keys named by `status` rules, lowercased; empty when there are none.
fn status_keys(request: &PageRequest) -> Vec<String> {
    equals_rules(request, "status")
        .flat_map(|v| {
            v.split(',')
                .map(|k| k.trim().to_lowercase())
                .filter(|k| !k.is_empty())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Resolve the `from` / `to` time tokens into a half-open UTC range `[from, to)`.
///
/// `from` takes `1h`, `24h`, `7d` or `30d` (that long before `now`) or a `YYYY-MM-DD` date
/// (its midnight, UTC). `to` takes a date and includes it, so the range ends at the next
/// midnight. Either end also takes an RFC 3339 timestamp, used as is (the dashboard chart
/// links a bucket this way, with `to` exclusive). `any`, blanks and anything that doesn't
/// parse leave that end open.
///
/// ```
/// use boson_backend::parse_time_range;
/// use chrono::{TimeZone, Utc};
///
/// let now = Utc.with_ymd_and_hms(2026, 11, 26, 12, 0, 0).unwrap();
/// let (from, to) = parse_time_range(Some("24h"), None, now);
/// assert_eq!(from, Some(Utc.with_ymd_and_hms(2026, 11, 25, 12, 0, 0).unwrap()));
/// assert_eq!(to, None);
/// let (from, to) = parse_time_range(Some("2026-11-01"), Some("2026-11-02"), now);
/// assert_eq!(from, Some(Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap()));
/// assert_eq!(to, Some(Utc.with_ymd_and_hms(2026, 11, 3, 0, 0, 0).unwrap()));
/// ```
#[must_use]
pub fn parse_time_range(
    from: Option<&str>,
    to: Option<&str>,
    now: DateTime<Utc>,
) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let from = from.and_then(|token| match token.trim() {
        "1h" => Some(now - Duration::hours(1)),
        "24h" => Some(now - Duration::hours(24)),
        "7d" => Some(now - Duration::days(7)),
        "30d" => Some(now - Duration::days(30)),
        other => parse_timestamp(other).or_else(|| parse_date(other)),
    });
    let to = to.and_then(|token| {
        let token = token.trim();
        parse_timestamp(token).or_else(|| parse_date(token).map(|day| day + Duration::days(1)))
    });
    (from, to)
}

fn parse_timestamp(token: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(token)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

fn parse_date(token: &str) -> Option<DateTime<Utc>> {
    NaiveDate::parse_from_str(token, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc())
}

/// Build the coordinator's [`RunFilter`] for a runs `DataTable` request.
///
/// Reads `equals` / `is` rules on `status` (status keys, comma-separated), `task_name` and
/// `job_id`, plus [`FROM_FIELD`] / [`TO_FIELD`] time tokens (see [`parse_time_range`]), all
/// combined with AND. `scope_job_id` (the URL's `?job=`) takes the place of a `job_id` rule. The quick search becomes `id_match`, lowercased: the whole run
/// id or its last characters. The server fn swaps it for `job_id` when the text is a whole job
/// id.
#[must_use]
pub fn run_filter_from_request(
    request: &PageRequest,
    scope_job_id: Option<&str>,
    now: DateTime<Utc>,
) -> RunFilter {
    let (started_from, started_to) = parse_time_range(
        first_rule(request, FROM_FIELD).as_deref(),
        first_rule(request, TO_FIELD).as_deref(),
        now,
    );
    let mut statuses: Vec<RunStatus> = status_keys(request)
        .iter()
        .filter_map(|k| parse_run_status(k))
        .collect();
    statuses.dedup();
    RunFilter {
        statuses,
        task: equals_rules(request, "task_name").next(),
        job_id: scope_job_id
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| equals_rules(request, "job_id").next()),
        started_from,
        started_to,
        id_match: quick_search_text(request).map(|q| q.to_lowercase()),
    }
}

/// Build the coordinator's [`JobFilter`] for a jobs `DataTable` request on `tab`.
///
/// The tab sets the statuses; `status` rules narrow them further. `task_name` and `pool` rules
/// and the [`FROM_FIELD`] / [`TO_FIELD`] creation range apply as they do for runs. Returns [`None`] when the
/// rules leave no status in the tab, so the caller can answer with an empty page (an empty
/// `statuses` list would match every status). Dropped jobs stay out. The quick search becomes
/// `id_match`, lowercased: the whole job id or its last characters.
#[must_use]
pub fn job_filter_from_request(
    request: &PageRequest,
    tab: JobsTab,
    now: DateTime<Utc>,
) -> Option<JobFilter> {
    let mut statuses = tab.statuses();
    let keys = status_keys(request);
    if !keys.is_empty() {
        statuses.retain(|s| keys.iter().any(|k| *k == s.to_string()));
        if statuses.is_empty() {
            return None;
        }
    }
    let (created_from, created_to) = parse_time_range(
        first_rule(request, FROM_FIELD).as_deref(),
        first_rule(request, TO_FIELD).as_deref(),
        now,
    );
    Some(JobFilter {
        statuses,
        task: equals_rules(request, "task_name").next(),
        pool: equals_rules(request, "pool").next(),
        created_from,
        created_to,
        id_match: quick_search_text(request).map(|q| q.to_lowercase()),
        include_dropped: false,
    })
}
