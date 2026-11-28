use boson_core::{PoolState, PoolStats, Run, RunStatus};
use chrono::{Duration, TimeZone, Utc};

use super::*;

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 24, 9, 30, 0).unwrap()
}

fn pool(name: &str, waiting: u64, added: u64, done: u64, oldest_mins: Option<i64>) -> PoolStats {
    PoolStats {
        pool: name.into(),
        waiting,
        running: 1,
        oldest_waiting_at: oldest_mins.map(|m| now() - Duration::minutes(m)),
        done_in_window: done,
        added_in_window: added,
        paused: false,
    }
}

fn task(name: &str, runs: u64, ok: u64, failed: u64, prior: u64) -> TaskFailureStats {
    TaskFailureStats {
        task: name.into(),
        runs_24h: runs,
        succeeded_24h: ok,
        failed_24h: failed,
        failed_prior_7d: prior,
        most_common_error: (failed > 0).then(|| "Ledger API refused the token (401)".into()),
    }
}

#[test]
fn queue_state_falling_behind_when_added_exceeds_done_and_waiting() {
    assert_eq!(
        queue_state(&pool("render", 41, 140, 96, Some(14))),
        QueueStateDto::FallingBehind
    );
}

#[test]
fn queue_state_keeping_up_when_nothing_waits_even_if_added_exceeds_done() {
    assert_eq!(
        queue_state(&pool("render", 0, 140, 96, None)),
        QueueStateDto::KeepingUp
    );
}

#[test]
fn queue_state_keeping_up_when_done_matches_added() {
    assert_eq!(
        queue_state(&pool("global", 5, 205, 205, Some(17))),
        QueueStateDto::KeepingUp
    );
}

#[test]
fn queue_state_paused_wins_over_falling_behind() {
    let mut p = pool("reports", 41, 140, 0, Some(3));
    p.paused = true;
    assert_eq!(queue_state(&p), QueueStateDto::Paused);
}

#[test]
fn queue_summary_carries_pause_actor_only_while_paused() {
    let mut p = pool("reports", 2, 2, 0, Some(1));
    p.paused = true;
    let state = PoolState {
        pool: "reports".into(),
        paused: true,
        changed_by: Some("Kenji Watanabe".into()),
        changed_at: now(),
    };
    let row = queue_summary(&p, Some(&state));
    assert_eq!(row.state, QueueStateDto::Paused);
    assert_eq!(row.paused_by.as_deref(), Some("Kenji Watanabe"));
    assert_eq!(row.owner, DEFAULT_OWNER_LABEL);

    p.paused = false;
    let resumed = PoolState {
        paused: false,
        ..state
    };
    let row = queue_summary(&p, Some(&resumed));
    assert_eq!(row.paused_by, None);
    assert_eq!(row.paused_at, None);
}

#[test]
fn sort_queues_puts_falling_behind_first_and_paused_last() {
    let mut paused = pool("a-paused", 0, 0, 0, None);
    paused.paused = true;
    let mut rows = vec![
        queue_summary(&paused, None),
        queue_summary(&pool("global", 5, 1, 2, Some(1)), None),
        queue_summary(&pool("render", 41, 140, 96, Some(14)), None),
    ];
    sort_queues(&mut rows);
    let names: Vec<_> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["render", "global", "a-paused"]);
}

#[test]
fn wait_label_words() {
    let n = now();
    assert_eq!(wait_label(n - Duration::seconds(20), n), "under a minute");
    assert_eq!(wait_label(n - Duration::minutes(14), n), "14 min");
    assert_eq!(wait_label(n - Duration::minutes(120), n), "2 h");
    assert_eq!(wait_label(n - Duration::minutes(125), n), "2 h 5 min");
    assert_eq!(wait_label(n - Duration::days(1), n), "1 day");
    assert_eq!(wait_label(n - Duration::days(3), n), "3 days");
    assert_eq!(wait_label(n + Duration::minutes(5), n), "under a minute");
    assert_eq!(oldest_waiting_label(None, n), "None waiting");
}

#[test]
fn oldest_waiting_warns_only_past_ten_minutes() {
    let n = now();
    assert!(!oldest_waiting_is_slow(None, n));
    assert!(!oldest_waiting_is_slow(Some(n - Duration::minutes(10)), n));
    assert!(oldest_waiting_is_slow(Some(n - Duration::minutes(11)), n));
    let pools = [pool("a", 1, 0, 0, Some(3)), pool("b", 1, 0, 0, Some(14))];
    assert_eq!(
        oldest_waiting_across(&pools),
        Some(n - Duration::minutes(14))
    );
}

#[test]
fn success_rate_has_one_decimal() {
    assert_eq!(success_rate_one_decimal(104, 95), "91.3%");
    assert_eq!(success_rate_one_decimal(860, 856), "99.5%");
    assert_eq!(success_rate_one_decimal(61, 61), "100.0%");
    assert_eq!(success_rate_one_decimal(0, 0), "No runs");
}

#[test]
fn most_common_error_picks_most_frequent_then_alphabetical() {
    assert_eq!(
        most_common_error(["b", "a", "b", " ", ""]),
        Some("b".to_string())
    );
    assert_eq!(most_common_error(["z", "a"]), Some("a".to_string()));
    assert_eq!(most_common_error(Vec::<&str>::new()), None);
}

#[test]
fn failing_more_than_usual_needs_minimum_and_above_average() {
    assert!(!failing_more_than_usual(2, 0));
    assert!(failing_more_than_usual(3, 0));
    assert!(failing_more_than_usual(9, 14));
    assert!(!failing_more_than_usual(3, 21));
    assert!(failing_more_than_usual(4, 21));
}

#[test]
fn needs_attention_empty_means_all_clear() {
    let pools = [pool("global", 5, 205, 212, Some(17))];
    let tasks = [task("email.send_digest", 61, 61, 0, 0)];
    assert_eq!(
        needs_attention_lines(&pools, &[], &tasks, now()),
        Vec::new()
    );
}

#[test]
fn needs_attention_orders_slow_queues_then_failures_by_count() {
    let pools = [
        pool("render-pool", 41, 140, 96, Some(14)),
        pool("older", 3, 9, 1, Some(40)),
        pool("global", 5, 205, 212, Some(17)),
    ];
    let failed = [
        FailedJobsByTask {
            task: "media.thumbnail".into(),
            count: 1,
        },
        FailedJobsByTask {
            task: "orders.sync_invoices".into(),
            count: 2,
        },
        FailedJobsByTask {
            task: "quiet".into(),
            count: 0,
        },
    ];
    let tasks = [
        task("orders.sync_invoices", 104, 95, 9, 0),
        task("media.thumbnail", 860, 856, 4, 70),
    ];
    let lines = needs_attention_lines(&pools, &failed, &tasks, now());
    let titles: Vec<_> = lines.iter().map(|l| l.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "older is falling behind",
            "render-pool is falling behind",
            "orders.sync_invoices failed 9 times today",
            "2 failed jobs in orders.sync_invoices",
            "1 failed job in media.thumbnail",
        ]
    );

    let slow = &lines[1];
    assert_eq!(slow.kind, AttentionKind::Slow);
    assert_eq!(
        slow.detail,
        "140 jobs added in the last hour, 96 done. The oldest has waited 14 min."
    );
    assert_eq!(slow.href, "/boson/queues/render-pool");
    assert_eq!(slow.link_label, "Open render-pool");

    let failing = &lines[2];
    assert_eq!(failing.kind, AttentionKind::Failed);
    assert_eq!(
        failing.detail,
        "Most failures: Ledger API refused the token (401)"
    );
    assert_eq!(
        failing.href,
        "/boson/runs?task=orders.sync_invoices&status=failed"
    );

    assert_eq!(
        lines[3].href,
        "/boson/jobs?tab=failed&task=orders.sync_invoices"
    );
    assert_eq!(lines[3].link_label, "Review failed jobs");
    assert_eq!(lines[4].link_label, "Review it");
    assert!(lines.iter().all(|l| l.owner == DEFAULT_OWNER_LABEL));
}

#[test]
fn failing_most_rows_sorted_and_formatted() {
    let rows = failing_most_rows(&[
        task("email.send_digest", 61, 61, 0, 0),
        task("orders.sync_invoices", 104, 95, 9, 0),
        task("media.thumbnail", 860, 856, 4, 0),
        task("idle", 0, 0, 0, 0),
    ]);
    let names: Vec<_> = rows.iter().map(|r| r.task.as_str()).collect();
    assert_eq!(
        names,
        [
            "orders.sync_invoices",
            "media.thumbnail",
            "email.send_digest",
            "idle"
        ]
    );
    assert_eq!(rows[0].success_rate, "91.3%");
    assert_eq!(rows[2].most_common_error, "None");
    assert_eq!(rows[3].success_rate, "No runs");
}

#[test]
fn ops_paths_encode_segments() {
    assert_eq!(boson_queue_path("a b/c"), "/boson/queues/a%20b%2Fc");
    assert_eq!(
        boson_queue_jobs_path("render"),
        "/boson/jobs?tab=active&pool=render"
    );
    assert_eq!(
        boson_failed_jobs_for_task_path("x&y"),
        "/boson/jobs?tab=failed&task=x%26y"
    );
    let from = Utc.with_ymd_and_hms(2026, 1, 2, 8, 0, 0).unwrap();
    assert_eq!(
        boson_runs_window_path(from, from + Duration::hours(4)),
        "/boson/runs?from=2026-01-02T08%3A00%3A00Z&to=2026-01-02T12%3A00%3A00Z"
    );
}

#[test]
fn validate_pool_name_rules() {
    assert!(validate_pool_name("render-pool").is_ok());
    assert_eq!(validate_pool_name("  "), Err(BosonIdError::EmptyPoolName));
    assert_eq!(validate_pool_name("a/b"), Err(BosonIdError::UnsafePoolName));
    assert_eq!(validate_pool_name(".."), Err(BosonIdError::UnsafePoolName));
    assert_eq!(
        validate_pool_name(&"p".repeat(MAX_POOL_NAME_CHARS + 1)),
        Err(BosonIdError::PoolNameTooLong)
    );
}

fn run_at(status: RunStatus, started_at: chrono::DateTime<Utc>) -> Run {
    Run {
        run_id: "r".into(),
        job_id: "j".into(),
        task_name: "t".into(),
        attempt: 1,
        status,
        started_at,
        finished_at: Some(started_at),
        duration_ms: Some(1),
        error_message: None,
    }
}

#[test]
fn run_series_has_three_groups_and_counts_by_bucket() {
    let n = Utc.with_ymd_and_hms(2026, 1, 2, 12, 0, 0).unwrap();
    let runs = [
        run_at(RunStatus::Success, n - Duration::hours(2)),
        run_at(RunStatus::Failed, n - Duration::hours(2)),
        run_at(RunStatus::Timeout, n - Duration::hours(2)),
        run_at(RunStatus::Canceled, n - Duration::hours(2)),
        run_at(RunStatus::Running, n - Duration::hours(2)),
        run_at(RunStatus::Success, n - Duration::hours(30)),
    ];
    let series = run_stats_series_from_runs(&runs, n, RANGE_SECS_24H);
    let ids: Vec<_> = series.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["succeeded", "failed", "timed_out_or_canceled"]);
    let starts = run_bucket_starts(n, RANGE_SECS_24H);
    assert!(series.iter().all(|s| s.points.len() == starts.len()));
    let bucket = starts
        .iter()
        .position(|t| *t == Utc.with_ymd_and_hms(2026, 1, 2, 8, 0, 0).unwrap())
        .unwrap();
    let values: Vec<f64> = series.iter().map(|s| s.points[bucket].value).collect();
    assert_eq!(values, [1.0, 1.0, 2.0]);
    let total: f64 = series
        .iter()
        .flat_map(|s| s.points.iter())
        .map(|p| p.value)
        .sum();
    assert!((total - 4.0).abs() < f64::EPSILON);
}

#[test]
fn run_series_keeps_empty_groups() {
    let n = Utc.with_ymd_and_hms(2026, 1, 2, 12, 0, 0).unwrap();
    let starts = run_bucket_starts(n, RANGE_SECS_7D);
    let series = run_series_from_counts(&starts, &[]);
    assert_eq!(series.len(), 3);
    assert!(series
        .iter()
        .all(|s| s.points.iter().all(|p| p.value == 0.0)));
}

#[test]
fn chart_bucket_window_parses_back_to_the_same_range() {
    let from = Utc.with_ymd_and_hms(2026, 1, 2, 8, 0, 0).unwrap();
    let to = from + Duration::hours(4);
    let (got_from, got_to) = parse_time_range(
        Some(&from.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        Some(&to.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        now(),
    );
    assert_eq!(got_from, Some(from));
    assert_eq!(got_to, Some(to));
}
