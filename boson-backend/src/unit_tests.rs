use boson_core::{Job, JobStatus, RetryPolicy, Run, RunStatus, TaskConfig};
use chrono::{TimeZone, Timelike, Utc};
use orbital_data::DataValue;
use orbital_paging::{FilterLogicWire, FilterQuery, FilterRuleParam, PageRequest};

use super::*;

fn sample_job_core(id: &str, task: &str, status: JobStatus) -> Job {
    Job {
        job_id: id.into(),
        task_name: task.into(),
        actor_json: serde_json::json!({}),
        params_json: serde_json::json!({}),
        priority: 3,
        pool: "global".into(),
        status,
        idempotency_key: None,
        created_at: Utc::now(),
        signature_hash: 0,
        attempt: 1,
        retried_from: None,
        dropped_at: None,
    }
}

fn sample_run_core(id: &str, job: &str, task: &str, status: RunStatus) -> Run {
    let now = Utc::now();
    Run {
        run_id: id.into(),
        job_id: job.into(),
        task_name: task.into(),
        attempt: 1,
        status,
        started_at: now,
        finished_at: Some(now),
        duration_ms: Some(10),
        error_message: None,
    }
}

fn sample_task(name: &str) -> TaskSummary {
    TaskSummary {
        name: name.into(),
        signature_json: "{}".into(),
        default_priority: 0,
        default_pool: "global".into(),
        effective_priority: 0,
        effective_pool: "global".into(),
        jobs_queued: 0,
        runs_total: 0,
        success_rate_pct: None,
        owner: None,
        access: String::new(),
    }
}

#[test]
fn validate_task_name_accepts_non_empty_happy_path() {
    validate_task_name("alpha").expect("non-empty");
    validate_task_name("  beta  ").expect("trimmed non-empty");
    validate_task_name("orders.sync").expect("dot in name");
}

#[test]
fn validate_task_name_rejects_blank_sad() {
    assert_eq!(
        validate_task_name("").expect_err("blank"),
        BosonIdError::EmptyTaskName
    );
    assert_eq!(
        validate_task_name("   ").expect_err("whitespace"),
        BosonIdError::EmptyTaskName
    );
    assert!(BosonIdError::EmptyTaskName.to_string().contains("required"));
}

#[test]
fn validate_task_name_rejects_slash_control_dotdot_sad() {
    assert_eq!(
        validate_task_name("a/b").expect_err("slash"),
        BosonIdError::UnsafeTaskName
    );
    assert_eq!(
        validate_task_name("a\\b").expect_err("backslash"),
        BosonIdError::UnsafeTaskName
    );
    assert_eq!(
        validate_task_name("a\tb").expect_err("control"),
        BosonIdError::UnsafeTaskName
    );
    assert_eq!(
        validate_task_name("..").expect_err("dotdot"),
        BosonIdError::UnsafeTaskName
    );
    assert_eq!(
        validate_task_name(".").expect_err("dot"),
        BosonIdError::UnsafeTaskName
    );
}

#[test]
fn validate_task_name_rejects_oversized_sad() {
    let oversized = "a".repeat(MAX_BOSON_ID_CHARS + 1);
    assert_eq!(
        validate_task_name(&oversized).expect_err("too long"),
        BosonIdError::TaskNameTooLong
    );
}

#[test]
fn validate_job_id_rejects_blank_sad() {
    assert_eq!(
        validate_job_id("").expect_err("blank"),
        BosonIdError::EmptyJobId
    );
}

#[test]
fn validate_job_id_rejects_slash_sad() {
    assert_eq!(
        validate_job_id("job/1").expect_err("slash"),
        BosonIdError::UnsafeJobId
    );
}

#[test]
fn validate_run_id_rejects_blank_sad() {
    assert_eq!(
        validate_run_id("  ").expect_err("whitespace"),
        BosonIdError::EmptyRunId
    );
}

#[test]
fn validate_run_id_rejects_control_sad() {
    assert_eq!(
        validate_run_id("run\nid").expect_err("control"),
        BosonIdError::UnsafeRunId
    );
}

#[test]
fn validate_job_id_accepts_id_happy_path() {
    validate_job_id("job-1").expect("id");
}

#[test]
fn validate_run_id_accepts_id_happy_path() {
    validate_run_id("run-1").expect("id");
}

#[test]
fn encode_ops_path_segment_encodes_slash_and_space_happy_path() {
    assert_eq!(encode_ops_path_segment("orders"), "orders");
    assert_eq!(encode_ops_path_segment("a/b"), "a%2Fb");
    assert_eq!(encode_ops_path_segment("a b"), "a%20b");
    assert_eq!(encode_ops_path_segment("a\\b"), "a%5Cb");
}

#[test]
fn boson_ops_paths_encode_segments_happy_path() {
    assert_eq!(boson_task_path("a/b"), "/boson/tasks/a%2Fb");
    assert_eq!(boson_task_config_path("a/b"), "/boson/tasks/a%2Fb/config");
    assert_eq!(boson_run_path("r/1"), "/boson/runs/r%2F1");
    assert_eq!(boson_runs_job_filter_path("j 1"), "/boson/runs?job=j%201");
}

#[test]
fn find_task_by_name_resolves_exact_happy_path() {
    let tasks = vec![sample_task("alpha"), sample_task("beta")];
    let found = find_task_by_name(&tasks, "beta").expect("listed");
    assert_eq!(found.name, "beta");
}

#[test]
fn find_task_by_name_unknown_is_none_sad() {
    let tasks = vec![sample_task("alpha")];
    assert!(find_task_by_name(&tasks, "__boson_missing_task__").is_none());
}

#[test]
fn find_job_by_id_resolves_exact_happy_path() {
    let jobs = vec![job_to_summary(sample_job_core(
        "j1",
        "alpha",
        JobStatus::Queued,
    ))];
    let found = find_job_by_id(&jobs, "j1").expect("listed");
    assert_eq!(found.task_name, "alpha");
}

#[test]
fn find_job_by_id_unknown_is_none_sad() {
    let jobs = vec![job_to_summary(sample_job_core(
        "j1",
        "alpha",
        JobStatus::Queued,
    ))];
    assert!(find_job_by_id(&jobs, "__boson_missing_job__").is_none());
}

#[test]
fn find_run_by_id_resolves_exact_happy_path() {
    let runs = vec![run_to_summary(sample_run_core(
        "r1",
        "j1",
        "alpha",
        RunStatus::Success,
    ))];
    let found = find_run_by_id(&runs, "r1").expect("listed");
    assert_eq!(found.job_id, "j1");
}

#[test]
fn find_run_by_id_unknown_is_none_sad() {
    let runs = vec![run_to_summary(sample_run_core(
        "r1",
        "j1",
        "alpha",
        RunStatus::Success,
    ))];
    assert!(find_run_by_id(&runs, "__boson_missing_run__").is_none());
}

#[test]
fn sort_tasks_by_name_orders_lexicographically_happy_path() {
    let mut tasks = vec![sample_task("zeta"), sample_task("alpha")];
    sort_tasks_by_name(&mut tasks);
    assert_eq!(tasks[0].name, "alpha");
    assert_eq!(tasks[1].name, "zeta");
}

#[test]
fn filter_tasks_by_query_happy_path() {
    let mut tasks = vec![sample_task("alpha"), sample_task("beta")];
    filter_tasks_by_query(&mut tasks, Some("alp"));
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].name, "alpha");
}

#[test]
fn filter_tasks_by_query_blank_keeps_all_sad() {
    let mut tasks = vec![sample_task("alpha"), sample_task("beta")];
    filter_tasks_by_query(&mut tasks, Some("   "));
    assert_eq!(tasks.len(), 2);
}

#[test]
fn parse_job_status_filter_maps_known_happy_path() {
    assert_eq!(parse_job_status_filter("queued"), Some(JobStatus::Queued));
    assert_eq!(parse_job_status_filter("failed"), Some(JobStatus::Failed));
}

#[test]
fn parse_job_status_filter_unknown_is_none_sad() {
    assert_eq!(parse_job_status_filter("unknown"), None);
    assert_eq!(parse_job_status_filter("QUEUED"), None);
    assert_eq!(parse_job_status_filter(""), None);
}

#[test]
fn job_and_run_to_summary_preserve_identity_happy_path() {
    let summary = job_to_summary(sample_job_core("j1", "alpha", JobStatus::Queued));
    assert_eq!(summary.job_id, "j1");
    assert_eq!(summary.status, JobStatusDto::Queued);
    assert_eq!(summary.priority, 3);

    let mut run = sample_run_core("r1", "j1", "alpha", RunStatus::Failed);
    run.error_message = Some("boom".into());
    run.duration_ms = Some(42);
    let run_summary = run_to_summary(run);
    assert_eq!(run_summary.run_id, "r1");
    assert_eq!(run_summary.status, RunStatusDto::Failed);
    assert_eq!(run_summary.duration_ms, Some(42));
    assert_eq!(run_summary.error_message.as_deref(), Some("boom"));
}

#[test]
fn aggregate_task_stats_groups_by_task_name_happy_path() {
    let jobs = vec![
        sample_job_core("j1", "alpha", JobStatus::Queued),
        sample_job_core("j2", "alpha", JobStatus::Running),
    ];
    let runs = vec![
        sample_run_core("r1", "j1", "alpha", RunStatus::Success),
        sample_run_core("r2", "j2", "beta", RunStatus::Failed),
    ];
    let stats = aggregate_task_stats(&jobs, &runs);
    assert_eq!(
        stats.get("alpha"),
        Some(&TaskStatsAgg {
            jobs_queued: 1,
            runs_total: 1,
            success_count: 1,
        })
    );
    assert_eq!(
        stats.get("beta"),
        Some(&TaskStatsAgg {
            jobs_queued: 0,
            runs_total: 1,
            success_count: 0,
        })
    );
}

#[test]
fn success_rate_pct_none_when_no_runs_sad() {
    assert_eq!(success_rate_pct(0, 0), None);
}

#[test]
fn success_rate_pct_computes_percent_happy_path() {
    assert_eq!(success_rate_pct(4, 3), Some(75.0));
}

#[test]
fn task_summary_from_parts_applies_config_happy_path() {
    let mut config = TaskConfig::default_for("alpha");
    config.priority = 9;
    config.pool = "urgent".into();
    config.retry_policy = RetryPolicy {
        max_attempts: 3,
        base_delay_ms: 100,
        backoff_multiplier: 2.0,
        max_delay_ms: 1000,
    };
    let summary = task_summary_from_parts(
        "alpha",
        r#"{"args":[]}"#,
        1,
        "global",
        &config,
        TaskStatsAgg {
            jobs_queued: 2,
            runs_total: 4,
            success_count: 2,
        },
    );
    assert_eq!(summary.effective_priority, 9);
    assert_eq!(summary.effective_pool, "urgent");
    assert_eq!(summary.success_rate_pct, Some(50.0));
}

#[test]
fn apply_task_config_update_merges_fields_happy_path() {
    let mut config = TaskConfig::default_for("alpha");
    let req = UpdateTaskConfigRequest {
        priority: Some(5),
        pool: Some("batch".into()),
        retry_policy: Some(RetryPolicyDto {
            max_attempts: 7,
            base_delay_ms: 50,
            backoff_multiplier: 1.5,
            max_delay_ms: 500,
        }),
        ..Default::default()
    };
    let now = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
    apply_task_config_update(&mut config, &req, now);
    let dto = task_config_to_dto(&config);
    assert_eq!(dto.priority, 5);
    assert_eq!(dto.pool, "batch");
    assert_eq!(dto.retry_policy.max_attempts, 7);
    assert_eq!(dto.updated_at, now.to_rfc3339());
}

#[test]
fn dashboard_stats_shape_happy_path() {
    let oldest = Utc::now();
    let stats = dashboard_stats(4, 1, 9, Some(oldest));
    assert_eq!(stats.jobs_waiting, 4);
    assert_eq!(stats.jobs_running, 1);
    assert_eq!(stats.failed_24h, 9);
    assert_eq!(stats.oldest_waiting_at, Some(oldest));
}

#[test]
fn run_bucket_granularity_switches_at_one_day_happy_path() {
    assert_eq!(
        run_bucket_granularity(86_400),
        RunBucketGranularity::FourHourly
    );
    assert_eq!(run_bucket_granularity(86_401), RunBucketGranularity::Daily);
}

#[test]
fn align_run_bucket_four_hourly_floors_happy_path() {
    let ts = Utc.with_ymd_and_hms(2026, 1, 1, 10, 45, 0).unwrap();
    let aligned = align_run_bucket(ts, RunBucketGranularity::FourHourly);
    assert_eq!(aligned.hour(), 8);
    assert_eq!(aligned.minute(), 0);
}

#[test]
fn run_stats_series_keeps_every_group_when_all_success_happy_path() {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
    let mut run = sample_run_core("r1", "j1", "alpha", RunStatus::Success);
    run.started_at = now - chrono::Duration::hours(1);
    let series = run_stats_series_from_runs(&[run], now, 86_400);
    assert_eq!(series.len(), 3);
    assert_eq!(series[0].id, "succeeded");
    assert!(series[0].points.iter().any(|p| p.value > 0.0));
    assert!(series[1].points.iter().all(|p| p.value == 0.0));
}

fn rules_request(items: Vec<(&str, &str)>, quick_search: Option<&str>) -> PageRequest {
    PageRequest {
        offset: 0,
        limit: 20,
        quick_search: quick_search.map(str::to_string),
        filter: Some(FilterQuery {
            logic: FilterLogicWire::And,
            items: items
                .into_iter()
                .map(|(field, value)| FilterRuleParam {
                    field: field.into(),
                    operator: "equals".into(),
                    value: DataValue::Text(value.into()),
                })
                .collect(),
        }),
        sort: None,
        cursor: None,
    }
}

#[test]
fn run_filter_reads_status_task_and_time_happy_path() {
    let now = Utc.with_ymd_and_hms(2026, 11, 26, 12, 0, 0).unwrap();
    let request = rules_request(
        vec![
            ("status", "failed,timeout"),
            ("task_name", "alpha"),
            ("from", "7d"),
        ],
        Some(" 0AB9 "),
    );
    let filter = run_filter_from_request(&request, None, now);
    assert_eq!(filter.statuses, vec![RunStatus::Failed, RunStatus::Timeout]);
    assert_eq!(filter.task.as_deref(), Some("alpha"));
    assert_eq!(filter.started_from, Some(now - chrono::Duration::days(7)));
    assert_eq!(filter.started_to, None);
    assert_eq!(filter.id_match.as_deref(), Some("0ab9"));
}

#[test]
fn run_filter_scope_wins_over_job_rule_happy_path() {
    let request = rules_request(vec![("job_id", "from-filter")], None);
    let filter = run_filter_from_request(&request, Some("from-scope"), Utc::now());
    assert_eq!(filter.job_id.as_deref(), Some("from-scope"));
    let filter = run_filter_from_request(&request, None, Utc::now());
    assert_eq!(filter.job_id.as_deref(), Some("from-filter"));
}

#[test]
fn job_filter_status_rule_narrows_tab_sad() {
    let request = rules_request(vec![("status", "queued")], None);
    let active = job_filter_from_request(&request, JobsTab::Active, Utc::now()).unwrap();
    assert_eq!(active.statuses, vec![JobStatus::Queued]);
    assert!(!active.include_dropped);
    assert_eq!(
        job_filter_from_request(&request, JobsTab::Failed, Utc::now()),
        None,
        "a status outside the tab matches nothing"
    );
}

#[test]
fn quick_search_text_rejects_blank_sad() {
    let request = PageRequest {
        offset: 0,
        limit: 20,
        quick_search: Some("  ".into()),
        filter: None,
        sort: None,
        cursor: None,
    };
    assert_eq!(quick_search_text(&request), None);
}

#[test]
fn jobs_tab_round_trips_keys_edge() {
    for tab in [JobsTab::Active, JobsTab::Failed, JobsTab::Finished] {
        assert_eq!(JobsTab::from_key(tab.key()), tab);
    }
    assert_eq!(JobsTab::from_key("queue"), JobsTab::Active);
    assert_eq!(
        JobsTab::Finished.statuses(),
        vec![JobStatus::Success, JobStatus::Canceled]
    );
}

#[test]
fn args_summary_shortens_values_edge() {
    let long = "x".repeat(40);
    let summary = args_summary(&serde_json::json!({"a": long, "b": {"k": 1}, "c": 3}));
    assert_eq!(summary, format!("a {}… · b {{…}}", "x".repeat(23)));
    assert_eq!(args_summary(&serde_json::json!("plain")), "plain");
    assert_eq!(args_summary(&serde_json::Value::Null), "");
}

#[test]
fn default_gluon_pool_rows_includes_global_happy_path() {
    let rows = default_gluon_pool_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "global");
}

fn pool_change(pool: &str) -> UpdateTaskConfigRequest {
    UpdateTaskConfigRequest {
        priority: None,
        pool: Some(pool.to_string()),
        retry_policy: None,
        ..Default::default()
    }
}

fn pion_offered() -> Vec<GluonPoolPickRow> {
    let mut rows = default_gluon_pool_rows();
    rows.push(GluonPoolPickRow {
        id: "boson-mve-a".to_string(),
        label: "boson-mve-a".to_string(),
        detail: "Boson workers on the MVE cell".to_string(),
    });
    rows
}

#[test]
fn validate_pool_offered_accepts_offered_pool_happy_path() {
    let offered = pion_offered();
    assert_eq!(
        validate_pool_offered(&pool_change(" boson-mve-a "), "global", &offered),
        Ok(())
    );
    let unchanged = UpdateTaskConfigRequest {
        priority: Some(3),
        pool: None,
        retry_policy: None,
        ..Default::default()
    };
    assert_eq!(
        validate_pool_offered(&unchanged, "global", &offered),
        Ok(())
    );
}

#[test]
fn reject_unknown_pool_id() {
    let offered = pion_offered();
    for pool in ["boson-mve-b", "general", "GLOBAL"] {
        assert_eq!(
            validate_pool_offered(&pool_change(pool), "global", &offered),
            Err(BosonInputError::UnknownPool),
            "{pool}"
        );
    }
    assert!(BosonInputError::UnknownPool
        .to_string()
        .starts_with("Invalid task config update:"));
}

#[test]
fn validate_pool_offered_keeps_retired_current_pool_sad() {
    assert_eq!(
        validate_pool_offered(
            &pool_change("retired-pool"),
            "retired-pool",
            &pion_offered()
        ),
        Ok(())
    );
}

#[test]
fn clamp_page_list_limit_caps_oversized_sad() {
    assert_eq!(clamp_page_list_limit(10_000), MAX_PAGE_LIST_LIMIT);
    assert_eq!(
        clamp_page_list_limit(MAX_PAGE_LIST_LIMIT),
        MAX_PAGE_LIST_LIMIT
    );
}

#[test]
fn clamp_page_list_limit_preserves_small_happy_path() {
    assert_eq!(clamp_page_list_limit(20), 20);
    assert_eq!(clamp_page_list_limit(0), 0);
}

#[test]
fn job_status_dto_serde_roundtrip_happy_path() {
    let status = JobStatusDto::Queued;
    let json = serde_json::to_string(&status).expect("serialize");
    assert_eq!(json, "\"queued\"");
    let back: JobStatusDto = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, JobStatusDto::Queued);
}

#[test]
fn validate_range_secs_accepts_ui_windows_happy_path() {
    validate_range_secs(RANGE_SECS_24H).expect("24h");
    validate_range_secs(RANGE_SECS_7D).expect("7d");
}

#[test]
fn validate_range_secs_rejects_other_values_sad() {
    assert_eq!(
        validate_range_secs(0).expect_err("zero"),
        BosonInputError::InvalidRangeSecs
    );
    assert_eq!(
        validate_range_secs(-1).expect_err("negative"),
        BosonInputError::InvalidRangeSecs
    );
    assert_eq!(
        validate_range_secs(86_401).expect_err("near 24h"),
        BosonInputError::InvalidRangeSecs
    );
    assert!(BosonInputError::InvalidRangeSecs
        .to_string()
        .starts_with("Invalid range_secs:"));
}

#[test]
fn validate_task_config_update_accepts_in_range_happy_path() {
    let req = UpdateTaskConfigRequest {
        priority: Some(10),
        pool: Some("global".into()),
        retry_policy: Some(RetryPolicyDto {
            max_attempts: 5,
            base_delay_ms: 100,
            backoff_multiplier: 2.0,
            max_delay_ms: 1_000,
        }),
        ..Default::default()
    };
    validate_task_config_update(&req).expect("in range");
    validate_task_config_update(&UpdateTaskConfigRequest {
        priority: None,
        pool: None,
        retry_policy: None,
        ..Default::default()
    })
    .expect("empty update");
}

#[test]
fn validate_task_config_update_rejects_out_of_range_sad() {
    assert_eq!(
        validate_task_config_update(&UpdateTaskConfigRequest {
            priority: Some(MAX_TASK_PRIORITY + 1),
            pool: None,
            retry_policy: None,
            ..Default::default()
        })
        .expect_err("priority"),
        BosonInputError::PriorityOutOfRange
    );
    assert_eq!(
        validate_task_config_update(&UpdateTaskConfigRequest {
            priority: None,
            pool: Some("a/b".into()),
            retry_policy: None,
            ..Default::default()
        })
        .expect_err("pool"),
        BosonInputError::InvalidPool
    );
    assert_eq!(
        validate_task_config_update(&UpdateTaskConfigRequest {
            priority: None,
            pool: Some(String::new()),
            retry_policy: None,
            ..Default::default()
        })
        .expect_err("blank pool"),
        BosonInputError::InvalidPool
    );
    assert_eq!(
        validate_task_config_update(&UpdateTaskConfigRequest {
            priority: None,
            pool: None,
            retry_policy: Some(RetryPolicyDto {
                max_attempts: 0,
                base_delay_ms: 1,
                backoff_multiplier: 2.0,
                max_delay_ms: 10,
            }),
            ..Default::default()
        })
        .expect_err("attempts"),
        BosonInputError::InvalidMaxAttempts
    );
    assert_eq!(
        validate_task_config_update(&UpdateTaskConfigRequest {
            priority: None,
            pool: None,
            retry_policy: Some(RetryPolicyDto {
                max_attempts: 3,
                base_delay_ms: MAX_RETRY_DELAY_MS + 1,
                backoff_multiplier: 2.0,
                max_delay_ms: 10,
            }),
            ..Default::default()
        })
        .expect_err("delay"),
        BosonInputError::InvalidRetryDelay
    );
    assert_eq!(
        validate_task_config_update(&UpdateTaskConfigRequest {
            priority: None,
            pool: None,
            retry_policy: Some(RetryPolicyDto {
                max_attempts: 3,
                base_delay_ms: 1,
                backoff_multiplier: 0.0,
                max_delay_ms: 10,
            }),
            ..Default::default()
        })
        .expect_err("multiplier"),
        BosonInputError::InvalidBackoffMultiplier
    );
    assert!(BosonInputError::PriorityOutOfRange
        .to_string()
        .starts_with("Invalid task config update:"));
}

#[test]
fn format_task_config_load_error_prefixes_cause_happy_path() {
    let msg = format_task_config_load_error("coordinator unavailable");
    assert!(msg.starts_with("Failed to load task config:"));
    assert!(msg.contains("coordinator unavailable"));
}

#[test]
fn format_task_config_load_error_never_empty_prefix_sad() {
    let msg = format_task_config_load_error("");
    assert_eq!(msg, "Failed to load task config: ");
    assert!(!msg.contains("default_for"));
}

fn retry(max_attempts: u32, base: u64, mult: f64, max: u64) -> RetryPolicyDto {
    RetryPolicyDto {
        max_attempts,
        base_delay_ms: base,
        backoff_multiplier: mult,
        max_delay_ms: max,
    }
}

#[test]
fn rate_limit_and_idempotency_round_trip_happy_path() {
    let mut config = TaskConfig::default_for("alpha");
    let req = UpdateTaskConfigRequest {
        rate_limit_policy: Some(RateLimitPolicyDto {
            max_in_flight: 10,
            max_enqueue_per_second: 5,
        }),
        idempotency_mode: Some(IdempotencyModeDto::AtLeastOnce),
        ..Default::default()
    };
    apply_task_config_update(&mut config, &req, Utc::now());
    let dto = task_config_to_dto(&config);
    assert_eq!(dto.rate_limit_policy.max_in_flight, 10);
    assert_eq!(dto.rate_limit_policy.max_enqueue_per_second, 5);
    assert_eq!(dto.idempotency_mode, IdempotencyModeDto::AtLeastOnce);
}

#[test]
fn idempotency_mode_unset_reads_exactly_once_sad() {
    let mut config = TaskConfig::default_for("alpha");
    config.idempotency_mode = None;
    assert_eq!(
        task_config_to_dto(&config).idempotency_mode,
        IdempotencyModeDto::ExactlyOnce
    );
    // An update without limits leaves them alone.
    config.rate_limit_policy.max_in_flight = 4;
    apply_task_config_update(&mut config, &UpdateTaskConfigRequest::default(), Utc::now());
    assert_eq!(config.rate_limit_policy.max_in_flight, 4);
    assert_eq!(config.idempotency_mode, None);
}

#[test]
fn short_id_keeps_tail_happy_path() {
    assert_eq!(short_id("00000000-0000-4000-9000-000000000412"), "…0412");
    assert_eq!(short_id("abc"), "abc");
}

#[test]
fn actor_label_maps_each_actor_happy_path() {
    let lookup = |id: &str| (id == "user:1").then(|| "Ada Mensah".to_string());
    assert_eq!(
        actor_label(&serde_json::json!({"System": {"operation": "x"}}), lookup),
        "System"
    );
    assert_eq!(
        actor_label(&serde_json::json!({"User": {"user_id": "user:1"}}), lookup),
        "Ada Mensah"
    );
    assert_eq!(
        actor_label(
            &serde_json::json!({"ServiceUser": {"service_name": "svc:orders"}}),
            lookup
        ),
        "orders app"
    );
}

#[test]
fn actor_label_unknown_shapes_sad() {
    let lookup = |_: &str| None;
    assert_eq!(
        actor_label(&serde_json::json!({"User": {"user_id": "user:9"}}), lookup),
        "user:9"
    );
    assert_eq!(actor_label(&serde_json::json!({}), lookup), "Unknown");
    assert_eq!(actor_label(&serde_json::json!("System"), lookup), "Unknown");
}

#[test]
fn job_label_adds_first_short_text_arg_happy_path() {
    let params = serde_json::json!({"batch": "batch-0042"});
    assert_eq!(job_label("job-0087", &params), "…0087 · batch batch-0042");
}

#[test]
fn job_label_skips_long_or_non_text_args_sad() {
    let long = "x".repeat(41);
    assert_eq!(
        job_label("job-0087", &serde_json::json!({"n": 3, "body": long})),
        "…0087"
    );
    assert_eq!(job_label("job-0087", &serde_json::json!(null)), "…0087");
}

#[test]
fn run_job_info_maps_job_happy_path() {
    let mut job = sample_job_core("job-0087", "alpha", JobStatus::Failed);
    job.actor_json = serde_json::json!({"System": {"operation": "seed"}});
    let info = run_job_info(&job, |_| None);
    assert_eq!(info.status, JobStatusDto::Failed);
    assert_eq!(info.pool, "global");
    assert_eq!(info.started_by, "System");
    assert_eq!(info.label, "…0087");
}

#[test]
fn format_duration_ms_units_happy_path() {
    assert_eq!(format_duration_ms(12), "12 ms");
    assert_eq!(format_duration_ms(30_000), "30.0 s");
    assert_eq!(format_duration_ms(125_000), "2 min 5 s");
    assert_eq!(format_duration_ms(120_000), "2 min");
}

#[test]
fn format_wait_ms_units_happy_path() {
    assert_eq!(format_wait_ms(1_000), "1 s");
    assert_eq!(format_wait_ms(1_500), "1.5 s");
    assert_eq!(format_wait_ms(300_000), "5 min");
    assert_eq!(format_wait_ms(3_600_000), "1 h");
}

#[test]
fn retry_schedule_sentence_lists_waits_happy_path() {
    assert_eq!(
        retry_schedule_sentence(&retry(3, 1_000, 2.0, 300_000)),
        "With these values: retry after 1 s, then 2 s. A job gives up after 3 attempts."
    );
    assert_eq!(
        retry_schedule_sentence(&retry(6, 1_000, 2.0, 3_000)),
        "With these values: retry after 1 s, then 2 s, then 3 s, and so on up to 3 s. A job gives up after 6 attempts."
    );
}

#[test]
fn retry_schedule_sentence_one_attempt_sad() {
    assert_eq!(
        retry_schedule_sentence(&retry(1, 1_000, 2.0, 300_000)),
        "With these values: no retries. A job gives up after 1 attempt."
    );
    assert_eq!(
        retry_wait_ms(&retry(3, 1_000, f64::INFINITY, 9_000), 2),
        9_000
    );
}

#[test]
fn settings_sentence_reads_config_happy_path() {
    let mut config = TaskConfig::default_for("alpha");
    config.pool = "global".into();
    config.priority = 1;
    config.retry_policy = RetryPolicy {
        max_attempts: 3,
        base_delay_ms: 1_000,
        backoff_multiplier: 2.0,
        max_delay_ms: 300_000,
    };
    config.rate_limit_policy.max_in_flight = 0;
    config.updated_at = Utc.with_ymd_and_hms(2026, 11, 18, 9, 0, 0).unwrap();
    assert_eq!(
        settings_sentence(&config),
        "Runs on global at priority 1, up to 3 attempts, waiting 1 s then doubling up to 5 min. Changed on Nov 18, 2026."
    );
}

#[test]
fn settings_sentence_single_attempt_with_limit_sad() {
    let mut config = TaskConfig::default_for("alpha");
    config.retry_policy.max_attempts = 1;
    config.rate_limit_policy.max_in_flight = 10;
    let s = settings_sentence(&config);
    assert!(s.contains("up to 1 attempt."), "{s}");
    assert!(s.contains("At most 10 waiting or running at once."), "{s}");
}

#[test]
fn task_arguments_from_signature_reads_properties_happy_path() {
    let sig = r#"{"description":"Syncs a batch.","properties":{"invoice_batch":{"type":"string","description":"The batch."}}}"#;
    let rows = task_arguments_from_signature(sig);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "invoice_batch");
    assert_eq!(rows[0].type_label, "Text");
    assert_eq!(rows[0].description.as_deref(), Some("The batch."));
    assert_eq!(
        task_description_from_signature(sig).as_deref(),
        Some("Syncs a batch.")
    );
}

#[test]
fn task_arguments_from_signature_bad_json_sad() {
    assert_eq!(
        task_arguments_from_signature("{}"),
        Vec::<TaskArgumentRow>::new()
    );
    assert_eq!(
        task_arguments_from_signature("not json"),
        Vec::<TaskArgumentRow>::new()
    );
    assert_eq!(
        task_arguments_from_signature(r#"{"args":[]}"#),
        Vec::<TaskArgumentRow>::new()
    );
    assert_eq!(
        task_description_from_signature(r#"{"description":"  "}"#),
        None
    );
}

#[test]
fn at_in_flight_limit_happy_path() {
    assert!(at_in_flight_limit(10, 10));
    assert!(!at_in_flight_limit(2, 10));
}

#[test]
fn at_in_flight_limit_zero_means_none_sad() {
    assert!(!at_in_flight_limit(50, 0));
}

#[test]
fn task_path_helpers_encode_happy_path() {
    assert_eq!(
        boson_task_settings_path("a b"),
        "/boson/tasks/a%20b?tab=settings"
    );
    assert_eq!(
        boson_task_failed_runs_path("a b"),
        "/boson/runs?task=a%20b&status=failed"
    );
    assert_eq!(boson_task_waiting_jobs_path("x"), "/boson/jobs?task=x");
    assert_eq!(boson_queue_pool_path("global"), "/boson/jobs?pool=global");
}

#[test]
fn validate_task_config_update_caps_attempts_at_twenty_sad() {
    assert_eq!(
        validate_task_config_update(&UpdateTaskConfigRequest {
            retry_policy: Some(retry(21, 1_000, 2.0, 1_000)),
            ..Default::default()
        })
        .unwrap_err(),
        BosonInputError::InvalidMaxAttempts
    );
    validate_task_config_update(&UpdateTaskConfigRequest {
        retry_policy: Some(retry(20, 1_000, 2.0, 1_000)),
        ..Default::default()
    })
    .expect("20 is allowed");
}

#[test]
fn filter_tasks_by_query_matches_owner_not_signature_happy_path() {
    let mut a = sample_task("alpha");
    a.owner = Some("Billing engineering".into());
    a.signature_json = r#"{"properties":{"zzz":{}}}"#.into();
    let mut tasks = vec![a, sample_task("beta")];
    filter_tasks_by_query(&mut tasks, Some("billing"));
    assert_eq!(tasks.len(), 1);
    filter_tasks_by_query(&mut tasks, Some("zzz"));
    assert_eq!(tasks, Vec::<TaskSummary>::new());
}

#[test]
fn parse_whole_number_field_accepts_range_happy_path() {
    assert_eq!(parse_whole_number_field(" 5 ", 1, 20), Ok(5));
    assert_eq!(parse_whole_number_field("-1000", -1000, 10_000), Ok(-1000));
}

#[test]
fn parse_whole_number_field_rejects_decimals_and_range_sad() {
    let msg = "Enter a whole number from 1 to 20.".to_string();
    assert_eq!(parse_whole_number_field("1.5", 1, 20), Err(msg.clone()));
    assert_eq!(parse_whole_number_field("21", 1, 20), Err(msg.clone()));
    assert_eq!(parse_whole_number_field("", 1, 20), Err(msg));
}

#[test]
fn parse_wait_seconds_field_converts_happy_path() {
    assert_eq!(parse_wait_seconds_field("1.5"), Ok(1_500));
    assert_eq!(parse_wait_seconds_field("300"), Ok(300_000));
    assert_eq!(ms_to_seconds_field(1_500), "1.5");
    assert_eq!(ms_to_seconds_field(300_000), "300");
}

#[test]
fn parse_wait_seconds_field_rejects_bad_input_sad() {
    assert!(parse_wait_seconds_field("-1").is_err());
    assert!(parse_wait_seconds_field("abc").is_err());
    assert!(parse_wait_seconds_field("86401").is_err());
}

#[test]
fn parse_multiplier_field_happy_path() {
    assert_eq!(parse_multiplier_field("2"), Ok(2.0));
}

#[test]
fn parse_multiplier_field_rejects_zero_sad() {
    assert!(parse_multiplier_field("0").is_err());
    assert!(parse_multiplier_field("inf").is_err());
}
