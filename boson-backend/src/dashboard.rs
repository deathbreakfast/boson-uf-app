//! Run-trend bucketing helpers for the Boson dashboard chart.

use std::collections::BTreeMap;

use chrono::{DateTime, Timelike, Utc};

use crate::types::{DashboardChartPoint, DashboardChartSeries};
use boson_core::{Run, RunStatus};

/// Bucket width for the dashboard run-outcomes chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunBucketGranularity {
    /// 4-hour buckets for the 24h range (6 x-axis labels).
    FourHourly,
    /// Daily buckets for the 7d range.
    Daily,
}

/// Chooses four-hourly buckets for ≤24h ranges and daily buckets otherwise.
#[must_use]
pub const fn run_bucket_granularity(range_secs: i64) -> RunBucketGranularity {
    if range_secs <= 86_400 {
        RunBucketGranularity::FourHourly
    } else {
        RunBucketGranularity::Daily
    }
}

/// Floors a timestamp to the start of its chart bucket.
#[must_use]
pub fn align_run_bucket(ts: DateTime<Utc>, bucket: RunBucketGranularity) -> DateTime<Utc> {
    let naive = ts.naive_utc();
    match bucket {
        RunBucketGranularity::FourHourly => {
            let aligned_hour = (naive.hour() / 4) * 4;
            let hour = naive
                .date()
                .and_hms_opt(aligned_hour, 0, 0)
                .unwrap_or(naive);
            DateTime::<Utc>::from_naive_utc_and_offset(hour, Utc)
        }
        RunBucketGranularity::Daily => {
            let day = naive.date().and_hms_opt(0, 0, 0).unwrap_or(naive);
            DateTime::<Utc>::from_naive_utc_and_offset(day, Utc)
        }
    }
}

/// Ensures every bucket between `since` and `now` exists (zero-filled).
pub fn fill_bucket_range(
    since: DateTime<Utc>,
    now: DateTime<Utc>,
    bucket: RunBucketGranularity,
    buckets: &mut BTreeMap<DateTime<Utc>, u32>,
) {
    let mut cursor = align_run_bucket(since, bucket);
    let end = align_run_bucket(now, bucket);
    let step = run_bucket_step(bucket);
    while cursor <= end {
        buckets.entry(cursor).or_insert(0);
        cursor += step;
    }
}

/// Step between bucket starts.
#[must_use]
pub const fn run_bucket_step(bucket: RunBucketGranularity) -> chrono::Duration {
    match bucket {
        RunBucketGranularity::FourHourly => chrono::Duration::hours(4),
        RunBucketGranularity::Daily => chrono::Duration::days(1),
    }
}

/// Start of every chart bucket in the window ending at `now`, oldest first.
#[must_use]
pub fn run_bucket_starts(now: DateTime<Utc>, range_secs: i64) -> Vec<DateTime<Utc>> {
    let bucket = run_bucket_granularity(range_secs);
    let mut map = BTreeMap::new();
    fill_bucket_range(
        now - chrono::Duration::seconds(range_secs),
        now,
        bucket,
        &mut map,
    );
    map.into_keys().collect()
}

/// Run outcome groups the dashboard chart stacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcomeGroup {
    /// `success`.
    Succeeded,
    /// `failed`.
    Failed,
    /// `timeout` or `canceled`.
    TimedOutOrCanceled,
}

impl RunOutcomeGroup {
    /// Every group, in chart order.
    pub const ALL: [Self; 3] = [Self::Succeeded, Self::Failed, Self::TimedOutOrCanceled];

    /// Stable series id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::TimedOutOrCanceled => "timed_out_or_canceled",
        }
    }

    /// Legend label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Succeeded => "Succeeded",
            Self::Failed => "Failed",
            Self::TimedOutOrCanceled => "Timed out or canceled",
        }
    }

    /// Run statuses counted in this group.
    #[must_use]
    pub fn statuses(self) -> Vec<RunStatus> {
        match self {
            Self::Succeeded => vec![RunStatus::Success],
            Self::Failed => vec![RunStatus::Failed],
            Self::TimedOutOrCanceled => vec![RunStatus::Timeout, RunStatus::Canceled],
        }
    }

    /// Group a finished status belongs to; `None` for runs still running.
    #[must_use]
    pub const fn of(status: RunStatus) -> Option<Self> {
        match status {
            RunStatus::Success => Some(Self::Succeeded),
            RunStatus::Failed => Some(Self::Failed),
            RunStatus::Timeout | RunStatus::Canceled => Some(Self::TimedOutOrCanceled),
            RunStatus::Running => None,
        }
    }
}

/// Builds the three chart series from per-bucket counts in [`RunOutcomeGroup::ALL`] order.
///
/// `counts[i]` belongs to `starts[i]`; missing entries count as zero. Every series is kept,
/// even when all its buckets are zero, so the legend and colours never shift.
#[must_use]
pub fn run_series_from_counts(
    starts: &[DateTime<Utc>],
    counts: &[[u32; 3]],
) -> Vec<DashboardChartSeries> {
    RunOutcomeGroup::ALL
        .iter()
        .enumerate()
        .map(|(g, group)| DashboardChartSeries {
            id: group.id().into(),
            label: group.label().into(),
            points: starts
                .iter()
                .enumerate()
                .map(|(i, ts)| DashboardChartPoint {
                    ts: *ts,
                    value: f64::from(counts.get(i).map_or(0, |c| c[g])),
                })
                .collect(),
        })
        .collect()
}

/// Builds the three chart series from a run list, bucketing by `started_at`.
#[must_use]
pub fn run_stats_series_from_runs(
    runs: &[Run],
    now: DateTime<Utc>,
    range_secs: i64,
) -> Vec<DashboardChartSeries> {
    let since = now - chrono::Duration::seconds(range_secs);
    let bucket = run_bucket_granularity(range_secs);
    let starts = run_bucket_starts(now, range_secs);
    let mut counts = vec![[0u32; 3]; starts.len()];
    for run in runs.iter().filter(|r| r.started_at >= since) {
        let Some(group) = RunOutcomeGroup::of(run.status) else {
            continue;
        };
        let ts = align_run_bucket(run.started_at, bucket);
        if let Ok(i) = starts.binary_search(&ts) {
            let g = RunOutcomeGroup::ALL
                .iter()
                .position(|x| *x == group)
                .unwrap_or(0);
            counts[i][g] += 1;
        }
    }
    run_series_from_counts(&starts, &counts)
}
