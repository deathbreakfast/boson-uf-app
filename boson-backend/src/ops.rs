//! Queue state, Needs attention lines and Failing most rows for the dashboard and Queues pages.
//!
//! Inputs are backend aggregates ([`PoolStats`], per-task counts); outputs are the DTOs the
//! pages render. Everything here is pure so the wording and ordering stay unit-testable.

use std::collections::BTreeMap;

use boson_core::{PoolState, PoolStats};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::JobSummary;
use crate::validate::encode_ops_path_segment;

/// Owner shown for task types and queues until per-team access lands.
pub const DEFAULT_OWNER_LABEL: &str = "Boson admins";

/// Window the Queues numbers and the falling-behind check use (one hour).
pub const QUEUE_STATS_WINDOW_SECS: i64 = 3_600;

/// Oldest waiting job age past which the dashboard card turns to a warning (ten minutes).
pub const OLDEST_WAITING_WARN_SECS: i64 = 600;

/// Failed runs in 24 hours a task type needs before it can count as failing more than usual.
pub const FAILING_MORE_THAN_USUAL_MIN: u64 = 3;

/// Rows the dashboard's Failing most table shows.
pub const FAILING_MOST_ROWS: usize = 10;

/// Waiting and running jobs the queue detail table lists.
pub const QUEUE_DETAIL_JOBS_CAP: usize = 50;

/// `/boson/queues` list href.
pub const BOSON_QUEUES_PATH: &str = "/boson/queues";

/// Whether a queue keeps up with the work arriving in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueStateDto {
    /// More jobs arrived than finished in the last hour, and some are waiting.
    FallingBehind,
    /// Jobs finish at least as fast as they arrive, or nothing is waiting.
    KeepingUp,
    /// Claims are paused; waiting jobs stay put.
    Paused,
}

impl QueueStateDto {
    /// Badge text.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FallingBehind => "Falling behind",
            Self::KeepingUp => "Keeping up",
            Self::Paused => "Paused",
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::FallingBehind => 0,
            Self::KeepingUp => 1,
            Self::Paused => 2,
        }
    }
}

/// One row of the Queues table (one Gluon pool).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueSummary {
    /// Pool name.
    pub name: String,
    /// Owner label.
    pub owner: String,
    /// Whether the reader's teams own the queue; "yours first" lists sort these rows first.
    #[serde(default)]
    pub yours: bool,
    /// Falling behind, keeping up, or paused.
    pub state: QueueStateDto,
    /// Jobs waiting.
    pub waiting: u64,
    /// Jobs running.
    pub running: u64,
    /// `created_at` of the oldest waiting job.
    pub oldest_waiting_at: Option<DateTime<Utc>>,
    /// Jobs finished in the last hour.
    pub done_last_hour: u64,
    /// Jobs added in the last hour.
    pub added_last_hour: u64,
    /// Who paused the queue, when it is paused.
    pub paused_by: Option<String>,
    /// When the queue was paused, when it is paused.
    pub paused_at: Option<DateTime<Utc>>,
    /// The reader's role on the queue ("Owner", "Can edit", "No access"), empty when the
    /// endpoint doesn't check it. Rows with "No access" carry no numbers.
    #[serde(default)]
    pub access: String,
}

/// Queue detail page payload: the summary row plus what is waiting and running.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueDetail {
    /// Same numbers as the list row.
    pub summary: QueueSummary,
    /// Waiting and running jobs, oldest first, capped at [`QUEUE_DETAIL_JOBS_CAP`].
    pub jobs: Vec<JobSummary>,
    /// Whether the reader holds Edit (pause and resume).
    #[serde(default)]
    pub can_edit: bool,
    /// Whether the reader holds Delete (purge).
    #[serde(default)]
    pub can_delete: bool,
}

/// Badge on a Needs attention line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttentionKind {
    /// Failed jobs or runs.
    Failed,
    /// A queue falling behind.
    Slow,
}

impl AttentionKind {
    /// Badge text.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Failed => "Failed",
            Self::Slow => "Slow",
        }
    }
}

/// One Needs attention line: what is wrong, in words, and the list behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedsAttentionLine {
    /// Failed or Slow.
    pub kind: AttentionKind,
    /// The problem in one sentence.
    pub title: String,
    /// Supporting numbers or the most common error.
    pub detail: String,
    /// Owner label.
    pub owner: String,
    /// Task type the line is about, for a failed line.
    #[serde(default)]
    pub task: Option<String>,
    /// Queue the line is about, for a falling-behind line.
    #[serde(default)]
    pub queue: Option<String>,
    /// Whether the reader's teams own the line's task type or queue.
    #[serde(default)]
    pub yours: bool,
    /// Filtered list behind the problem.
    pub href: String,
    /// Link text.
    pub link_label: String,
}

/// Failed-job count for one task type (dashboard input).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedJobsByTask {
    /// Task type name.
    pub task: String,
    /// Jobs in `failed` that wait for Retry or Drop.
    pub count: u64,
}

/// Run outcomes for one task type over the last 24 hours (dashboard input).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFailureStats {
    /// Task type name.
    pub task: String,
    /// Runs started in the last 24 hours.
    pub runs_24h: u64,
    /// Of those, runs that succeeded.
    pub succeeded_24h: u64,
    /// Of those, runs that failed.
    pub failed_24h: u64,
    /// Failed runs in the seven days before the last 24 hours.
    pub failed_prior_7d: u64,
    /// Most common error among the last 24 hours' failed runs.
    pub most_common_error: Option<String>,
}

/// One row of the dashboard's Failing most table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailingTaskRow {
    /// Task type name.
    pub task: String,
    /// Owner label.
    pub owner: String,
    /// Whether the reader's teams own the task type.
    #[serde(default)]
    pub yours: bool,
    /// Failed runs in the last 24 hours.
    pub failed_24h: u64,
    /// Runs in the last 24 hours.
    pub runs_24h: u64,
    /// Success rate with one decimal (`"91.3%"`), or `"No runs"`.
    pub success_rate: String,
    /// Most common error, or `"None"`.
    pub most_common_error: String,
}

/// Falling behind means jobs added in the window exceed jobs finished and something waits.
/// A paused queue is Paused whatever its numbers.
#[must_use]
pub const fn queue_state(stats: &PoolStats) -> QueueStateDto {
    if stats.paused {
        QueueStateDto::Paused
    } else if stats.added_in_window > stats.done_in_window && stats.waiting > 0 {
        QueueStateDto::FallingBehind
    } else {
        QueueStateDto::KeepingUp
    }
}

/// Builds a Queues row from pool stats and its stored pause state.
#[must_use]
pub fn queue_summary(stats: &PoolStats, pause: Option<&PoolState>) -> QueueSummary {
    let paused = pause.filter(|p| p.paused && stats.paused);
    QueueSummary {
        name: stats.pool.clone(),
        owner: DEFAULT_OWNER_LABEL.to_string(),
        yours: false,
        state: queue_state(stats),
        waiting: stats.waiting,
        running: stats.running,
        oldest_waiting_at: stats.oldest_waiting_at,
        done_last_hour: stats.done_in_window,
        added_last_hour: stats.added_in_window,
        paused_by: paused.and_then(|p| p.changed_by.clone()),
        paused_at: paused.map(|p| p.changed_at),
        access: String::new(),
    }
}

/// Orders Queues rows: falling behind first, then keeping up, then paused; by name within each.
pub fn sort_queues(rows: &mut [QueueSummary]) {
    rows.sort_by(|a, b| {
        a.state
            .rank()
            .cmp(&b.state.rank())
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// Oldest waiting time across every pool.
#[must_use]
pub fn oldest_waiting_across(pools: &[PoolStats]) -> Option<DateTime<Utc>> {
    pools.iter().filter_map(|p| p.oldest_waiting_at).min()
}

/// Whether the oldest waiting job has waited longer than [`OLDEST_WAITING_WARN_SECS`].
#[must_use]
pub fn oldest_waiting_is_slow(oldest: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    oldest.is_some_and(|t| (now - t).num_seconds() > OLDEST_WAITING_WARN_SECS)
}

/// How long something has waited, in words: `"under a minute"`, `"14 min"`, `"2 h 5 min"`,
/// `"3 days"`.
#[must_use]
pub fn wait_label(since: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let mins = (now - since).num_minutes().max(0);
    if mins < 1 {
        "under a minute".to_string()
    } else if mins < 60 {
        format!("{mins} min")
    } else if mins < 24 * 60 {
        let (h, m) = (mins / 60, mins % 60);
        if m == 0 {
            format!("{h} h")
        } else {
            format!("{h} h {m} min")
        }
    } else {
        let days = mins / (24 * 60);
        if days == 1 {
            "1 day".to_string()
        } else {
            format!("{days} days")
        }
    }
}

/// Oldest waiting cell text: a [`wait_label`], or `"None waiting"`.
#[must_use]
pub fn oldest_waiting_label(oldest: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    oldest.map_or_else(|| "None waiting".to_string(), |t| wait_label(t, now))
}

/// Success rate with one decimal, or `"No runs"` when there were none.
#[must_use]
pub fn success_rate_one_decimal(runs: u64, succeeded: u64) -> String {
    if runs == 0 {
        return "No runs".to_string();
    }
    #[allow(clippy::cast_precision_loss)]
    let pct = succeeded.min(runs) as f64 / runs as f64 * 100.0;
    format!("{pct:.1}%")
}

/// Most frequent error message; ties go to the alphabetically first so the result is stable.
#[must_use]
pub fn most_common_error<'a>(errors: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
    for e in errors {
        let e = e.trim();
        if !e.is_empty() {
            *counts.entry(e).or_insert(0) += 1;
        }
    }
    let max = counts.values().copied().max()?;
    counts
        .into_iter()
        .find(|&(_, n)| n == max)
        .map(|(e, _)| e.to_string())
}

/// A task type fails more than usual when its failed runs in the last 24 hours reach
/// [`FAILING_MORE_THAN_USUAL_MIN`] and exceed its daily average over the seven days before.
#[must_use]
pub const fn failing_more_than_usual(failed_24h: u64, failed_prior_7d: u64) -> bool {
    failed_24h >= FAILING_MORE_THAN_USUAL_MIN && failed_24h * 7 > failed_prior_7d
}

fn plural(n: u64, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Needs attention lines, worst first.
///
/// Queues falling behind come first (longest wait first), then failed-job and failing-task
/// lines by count. An empty list means All clear.
#[must_use]
pub fn needs_attention_lines(
    pools: &[PoolStats],
    failed_jobs: &[FailedJobsByTask],
    tasks: &[TaskFailureStats],
    now: DateTime<Utc>,
) -> Vec<NeedsAttentionLine> {
    let mut slow: Vec<&PoolStats> = pools
        .iter()
        .filter(|p| queue_state(p) == QueueStateDto::FallingBehind)
        .collect();
    slow.sort_by(|a, b| {
        a.oldest_waiting_at
            .cmp(&b.oldest_waiting_at)
            .then_with(|| a.pool.cmp(&b.pool))
    });
    let mut lines: Vec<NeedsAttentionLine> = slow
        .into_iter()
        .map(|p| {
            let wait = p.oldest_waiting_at.map_or_else(String::new, |t| {
                format!(" The oldest has waited {}.", wait_label(t, now))
            });
            NeedsAttentionLine {
                kind: AttentionKind::Slow,
                title: format!("{} is falling behind", p.pool),
                detail: format!(
                    "{} added in the last hour, {} done.{wait}",
                    plural(p.added_in_window, "job", "jobs"),
                    p.done_in_window
                ),
                owner: DEFAULT_OWNER_LABEL.to_string(),
                task: None,
                queue: Some(p.pool.clone()),
                yours: false,
                href: boson_queue_path(&p.pool),
                link_label: format!("Open {}", p.pool),
            }
        })
        .collect();

    let mut failures: Vec<(u64, NeedsAttentionLine)> = Vec::new();
    for f in failed_jobs.iter().filter(|f| f.count > 0) {
        let (detail, link) = if f.count == 1 {
            (
                "It used every attempt and waits for Retry or Drop.",
                "Review it",
            )
        } else {
            (
                "They used every attempt and wait for Retry or Drop.",
                "Review failed jobs",
            )
        };
        failures.push((
            f.count,
            NeedsAttentionLine {
                kind: AttentionKind::Failed,
                title: format!(
                    "{} in {}",
                    plural(f.count, "failed job", "failed jobs"),
                    f.task
                ),
                detail: detail.to_string(),
                owner: DEFAULT_OWNER_LABEL.to_string(),
                task: Some(f.task.clone()),
                queue: None,
                yours: false,
                href: boson_failed_jobs_for_task_path(&f.task),
                link_label: link.to_string(),
            },
        ));
    }
    for t in tasks
        .iter()
        .filter(|t| failing_more_than_usual(t.failed_24h, t.failed_prior_7d))
    {
        let detail = t.most_common_error.as_deref().map_or_else(
            || "No error message was recorded.".to_string(),
            |e| format!("Most failures: {e}"),
        );
        failures.push((
            t.failed_24h,
            NeedsAttentionLine {
                kind: AttentionKind::Failed,
                title: format!("{} failed {} times today", t.task, t.failed_24h),
                detail,
                owner: DEFAULT_OWNER_LABEL.to_string(),
                task: Some(t.task.clone()),
                queue: None,
                yours: false,
                href: boson_failed_runs_for_task_path(&t.task),
                link_label: "See its failed runs".to_string(),
            },
        ));
    }
    failures.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.title.cmp(&b.1.title)));
    lines.extend(failures.into_iter().map(|(_, line)| line));
    lines
}

/// Failing most rows: most failed runs first, then most runs, then name.
#[must_use]
pub fn failing_most_rows(tasks: &[TaskFailureStats]) -> Vec<FailingTaskRow> {
    let mut sorted: Vec<&TaskFailureStats> = tasks.iter().collect();
    sorted.sort_by(|a, b| {
        b.failed_24h
            .cmp(&a.failed_24h)
            .then_with(|| b.runs_24h.cmp(&a.runs_24h))
            .then_with(|| a.task.cmp(&b.task))
    });
    sorted
        .into_iter()
        .take(FAILING_MOST_ROWS)
        .map(|t| FailingTaskRow {
            task: t.task.clone(),
            owner: DEFAULT_OWNER_LABEL.to_string(),
            yours: false,
            failed_24h: t.failed_24h,
            runs_24h: t.runs_24h,
            success_rate: success_rate_one_decimal(t.runs_24h, t.succeeded_24h),
            most_common_error: t
                .most_common_error
                .clone()
                .unwrap_or_else(|| "None".to_string()),
        })
        .collect()
}

/// `/boson/queues/{encoded}` detail href.
#[must_use]
pub fn boson_queue_path(pool: &str) -> String {
    format!("{BOSON_QUEUES_PATH}/{}", encode_ops_path_segment(pool))
}

/// Jobs list, Active tab, for one queue (`pool` narrows it, as on the task page's queue link).
#[must_use]
pub fn boson_queue_jobs_path(pool: &str) -> String {
    format!(
        "/boson/jobs?tab=active&pool={}",
        encode_ops_path_segment(pool)
    )
}

/// Jobs list, Failed tab, for one task type (`task` narrows it once the Jobs page reads it).
#[must_use]
pub fn boson_failed_jobs_for_task_path(task: &str) -> String {
    format!(
        "/boson/jobs?tab=failed&task={}",
        encode_ops_path_segment(task)
    )
}

/// Runs list filtered to failed runs of one task type.
#[must_use]
pub fn boson_failed_runs_for_task_path(task: &str) -> String {
    format!(
        "/boson/runs?task={}&status=failed",
        encode_ops_path_segment(task)
    )
}

/// Runs list for one chart bucket, `from` inclusive and `to` exclusive.
#[must_use]
pub fn boson_runs_window_path(from: DateTime<Utc>, to: DateTime<Utc>) -> String {
    let fmt = |t: DateTime<Utc>| {
        encode_ops_path_segment(&t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
    };
    format!("/boson/runs?from={}&to={}", fmt(from), fmt(to))
}
