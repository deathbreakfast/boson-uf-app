//! Integration contracts for the `DataTable` page-query helpers behind
//! `list_jobs_datatable_page` / `list_runs_datatable_page`: rules and quick search become the
//! coordinator's `RunFilter` / `JobFilter`, and pages follow keyset cursors at any history size.

#![allow(missing_docs)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;

use boson_backend::{
    clamp_page_list_limit, extract_status_filter, job_filter_from_request, job_status_key,
    quick_search_text, resolve_job_filter, run_filter_from_request, run_status_key,
    runs_list_start, runs_page_from_rows, JobStatusDto, JobsTab, ListStart, RunStatusDto,
    MAX_PAGE_LIST_LIMIT,
};
use boson_core::{page_runs_after, JobStatus, Run, RunStatus};
use chrono::{DateTime, Duration, TimeZone, Utc};
use orbital_data::DataValue;
use orbital_paging::{FilterLogicWire, FilterQuery, FilterRuleParam, PageRequest};

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 26, 12, 0, 0).unwrap()
}

fn rule(field: &str, value: &str) -> FilterRuleParam {
    FilterRuleParam {
        field: field.into(),
        operator: "is".into(),
        value: DataValue::Text(value.into()),
    }
}

fn request(rules: Vec<FilterRuleParam>, quick_search: Option<&str>) -> PageRequest {
    let filter = (!rules.is_empty()).then_some(FilterQuery {
        items: rules,
        logic: FilterLogicWire::And,
    });
    PageRequest::with_query(0, 20, None, filter, quick_search.map(str::to_string))
}

#[test]
fn run_filter_maps_toolbar_rules_happy_path() {
    let req = request(
        vec![
            rule("status", "failed"),
            rule("task_name", "orders.sync_invoices"),
            rule("from", "2026-11-01"),
            rule("to", "2026-11-03"),
        ],
        Some("0999"),
    );
    let filter = run_filter_from_request(&req, None, now());
    assert_eq!(filter.statuses, vec![RunStatus::Failed]);
    assert_eq!(filter.task.as_deref(), Some("orders.sync_invoices"));
    assert_eq!(
        filter.started_from,
        Some(Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap())
    );
    assert_eq!(
        filter.started_to,
        Some(Utc.with_ymd_and_hms(2026, 11, 4, 0, 0, 0).unwrap()),
        "the to date is included"
    );
    assert_eq!(filter.id_suffix(), Some("0999"));
}

#[test]
fn run_filter_ignores_unknown_status_and_time_sad_path() {
    let req = request(
        vec![rule("status", "exploded"), rule("from", "yesterday")],
        None,
    );
    let filter = run_filter_from_request(&req, None, now());
    assert!(
        filter.statuses.is_empty(),
        "unknown status keys add nothing"
    );
    assert_eq!(filter.started_from, None);
    assert_eq!(filter.id_match, None);
}

#[test]
fn run_filter_search_matches_whole_id_or_last_characters_edge() {
    let req = request(vec![], Some("A0420"));
    let filter = run_filter_from_request(&req, None, now());
    let mut run = Run::new("job", "t", 1);
    run.run_id = "00000000-0000-4000-8000-00000000a0420".into();
    assert!(
        filter.matches(&run),
        "search is lowercased and matches the id's end"
    );
    run.run_id = "a0420-0000-4000-8000-000000000001".into();
    assert!(!filter.matches(&run), "a match in the middle doesn't count");
}

#[test]
fn job_filter_per_tab_happy_path() {
    let req = request(vec![rule("task_name", "media.thumbnail")], Some("0101"));
    let active = job_filter_from_request(&req, JobsTab::Active, now()).unwrap();
    assert_eq!(active.statuses, vec![JobStatus::Queued, JobStatus::Running]);
    assert_eq!(active.task.as_deref(), Some("media.thumbnail"));
    assert_eq!(active.id_suffix(), Some("0101"));
    let failed = job_filter_from_request(&req, JobsTab::Failed, now()).unwrap();
    assert_eq!(failed.statuses, vec![JobStatus::Failed]);
    assert!(!failed.include_dropped, "dropped jobs leave Failed");
}

#[test]
fn job_filter_status_outside_tab_is_none_sad_path() {
    let req = request(vec![rule("status", "success")], None);
    assert!(job_filter_from_request(&req, JobsTab::Active, now()).is_none());
    let finished = job_filter_from_request(&req, JobsTab::Finished, now()).unwrap();
    assert_eq!(finished.statuses, vec![JobStatus::Success]);
}

/// 50,001 runs, one per second, paged newest first through the same calls
/// `list_runs_datatable_page` makes, with `page_runs_after` standing in for the coordinator's
/// keyset read. Every run shows up once and in order, well past the old 50,000-row scan cap.
#[test]
fn runs_page_past_fifty_thousand_by_cursor_happy_path() {
    const N: usize = 50_001;
    const PAGE: u32 = 100;
    let base = now() - Duration::days(1);
    let all: Vec<Run> = (0..N)
        .map(|i| {
            let mut r = Run::new(if i % 2 == 0 { "job-a" } else { "job-b" }, "t", 1);
            r.run_id = format!("run-{i:06}");
            r.started_at = base + Duration::milliseconds(i64::try_from(i).unwrap());
            r
        })
        .collect();
    // "Last 24 hours" and the same toolbar rules on every page, as the Runs table sends them.
    let rules = vec![rule("from", "30d")];
    let total = {
        let req = request(rules.clone(), None);
        let filter = run_filter_from_request(&req, None, now());
        all.iter().filter(|r| filter.matches(r)).count()
    };
    assert_eq!(total, N, "the first page's count covers every run");

    let mut seen = Vec::with_capacity(N);
    let mut cursor: Option<String> = None;
    let mut offset = 0u32;
    loop {
        let filter_q = FilterQuery {
            items: rules.clone(),
            logic: FilterLogicWire::And,
        };
        let req = PageRequest::with_query(offset, PAGE, None, Some(filter_q), None)
            .with_cursor(cursor.clone());
        let filter = run_filter_from_request(&req, None, now());
        let start = runs_list_start(&req, None);
        let key = match &start {
            ListStart::After(key) => key.clone(),
            ListStart::Offset(_) => panic!("every page after the first carries a cursor"),
        };
        let rows: Vec<Run> =
            page_runs_after(all.iter(), &filter, key.as_ref(), (PAGE + 1) as usize)
                .into_iter()
                .cloned()
                .collect();
        let page = runs_page_from_rows(rows, &req, None, PAGE, &start, None);
        seen.extend(page.items.iter().map(|r| r.run_id.clone()));
        if !page.has_more {
            break;
        }
        cursor = page.next_cursor.clone();
        assert!(
            cursor.is_some(),
            "a page with more rows hands back a cursor"
        );
        offset += PAGE;
    }
    assert_eq!(seen.len(), N);
    assert_eq!(seen.iter().collect::<HashSet<_>>().len(), N, "no run twice");
    assert_eq!(
        seen.first().map(String::as_str),
        Some("run-050000"),
        "newest first"
    );
    assert_eq!(seen.last().map(String::as_str), Some("run-000000"));
}

#[test]
fn runs_cursor_from_other_filter_restarts_sad_path() {
    let first = PageRequest::with_query(
        0,
        20,
        None,
        Some(FilterQuery {
            items: vec![rule("status", "failed")],
            logic: FilterLogicWire::And,
        }),
        None,
    );
    let rows: Vec<Run> = (0..30)
        .map(|i| {
            let mut r = Run::new("job", "t", 1);
            r.run_id = format!("run-{i:03}");
            r.status = RunStatus::Failed;
            r
        })
        .collect();
    let start = runs_list_start(&first, None);
    let page = runs_page_from_rows(rows[..21].to_vec(), &first, None, 20, &start, None);
    let token = page.next_cursor.expect("cursor");
    // Same cursor, but the status Select changed: the cursor no longer fits the query.
    let changed = PageRequest::with_query(
        20,
        20,
        None,
        Some(FilterQuery {
            items: vec![rule("status", "success")],
            logic: FilterLogicWire::And,
        }),
        None,
    )
    .with_cursor(Some(token));
    assert_eq!(runs_list_start(&changed, None), ListStart::Offset(20));
}

#[test]
fn extract_status_filter_reads_equals_happy_path() {
    let request = PageRequest {
        offset: 0,
        limit: 20,
        quick_search: None,
        filter: Some(FilterQuery {
            logic: FilterLogicWire::And,
            items: vec![FilterRuleParam {
                field: "status".into(),
                operator: "equals".into(),
                value: DataValue::Text("Queued".into()),
            }],
        }),
        sort: None,
        cursor: None,
    };
    assert_eq!(extract_status_filter(&request), Some("queued".into()));
}

#[test]
fn extract_status_filter_non_status_is_none_sad() {
    let request = PageRequest {
        offset: 0,
        limit: 20,
        quick_search: None,
        filter: Some(FilterQuery {
            logic: FilterLogicWire::And,
            items: vec![FilterRuleParam {
                field: "pool".into(),
                operator: "equals".into(),
                value: DataValue::Text("global".into()),
            }],
        }),
        sort: None,
        cursor: None,
    };
    assert_eq!(extract_status_filter(&request), None);
}

#[test]
fn resolve_job_filter_prefers_scope_happy_path() {
    let request = PageRequest {
        offset: 0,
        limit: 20,
        quick_search: None,
        filter: Some(FilterQuery {
            logic: FilterLogicWire::And,
            items: vec![FilterRuleParam {
                field: "job_id".into(),
                operator: "equals".into(),
                value: DataValue::Text("from-filter".into()),
            }],
        }),
        sort: None,
        cursor: None,
    };
    assert_eq!(
        resolve_job_filter(Some("from-scope".into()), &request),
        Some("from-scope".into())
    );
}

#[test]
fn resolve_job_filter_falls_back_to_filter_happy_path() {
    let request = PageRequest {
        offset: 0,
        limit: 20,
        quick_search: None,
        filter: Some(FilterQuery {
            logic: FilterLogicWire::And,
            items: vec![FilterRuleParam {
                field: "job_id".into(),
                operator: "equals".into(),
                value: DataValue::Text("from-filter".into()),
            }],
        }),
        sort: None,
        cursor: None,
    };
    assert_eq!(
        resolve_job_filter(None, &request),
        Some("from-filter".into())
    );
    assert_eq!(
        resolve_job_filter(Some(String::new()), &request),
        Some("from-filter".into())
    );
}

#[test]
fn quick_search_text_rejects_blank_sad() {
    let request = PageRequest {
        offset: 0,
        limit: 20,
        quick_search: Some("   ".into()),
        filter: None,
        sort: None,
        cursor: None,
    };
    assert_eq!(quick_search_text(&request), None);
}

#[test]
fn status_keys_map_variants_happy_path() {
    assert_eq!(job_status_key(JobStatusDto::Queued), "queued");
    assert_eq!(run_status_key(RunStatusDto::Timeout), "timeout");
}

#[test]
fn clamp_page_list_limit_caps_at_max_happy_path() {
    assert_eq!(clamp_page_list_limit(50), 50);
    assert_eq!(
        clamp_page_list_limit(MAX_PAGE_LIST_LIMIT + 500),
        MAX_PAGE_LIST_LIMIT
    );
}
