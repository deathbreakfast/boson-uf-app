//! Cursor paging for the jobs and runs `DataTable` server functions.
//!
//! `list_jobs_datatable_page` and `list_runs_datatable_page` take one of two paths:
//!
//! - **Cursor path** — when every filter can be pushed down to the coordinator (jobs: no quick
//!   search and only `status` rules; runs: no quick search and at most one `job_id` equals rule
//!   when the URL has no job scope). The first page and every page that carries a valid cursor
//!   read with the coordinator's keyset methods (`list_jobs_after` in `created_at ASC, job_id
//!   ASC` order, `list_runs_after` in `started_at DESC, run_id DESC` order). Each page with more
//!   rows returns a `next_cursor` holding the last row's key.
//! - **Offset path** — a request past the first page with no cursor, or with a cursor that does
//!   not decode for this query (another filter, page size or job scope, a garbage token, or a key
//!   that does not parse), reads with `list_jobs` / `list_runs` at `request.offset` and returns
//!   no `next_cursor`. Quick search and other filters stay on the bounded in-memory scan in the
//!   server functions and never use a cursor.
//!
//! The cursor key is `"<created_at>|<job_id>"` for jobs and `"<started_at>|<run_id>"` for runs,
//! with the timestamp in RFC 3339 at nanosecond precision. `position` holds the offset the next
//! page would have had; reads ignore it. A cursor only says where to continue: the coordinator
//! read still applies the status and job filters from the request.

use boson_core::{Job, JobKey, Run, RunKey};
use chrono::{DateTime, SecondsFormat, Utc};
use orbital_paging::{Page, PageCursor, PageRequest};

use crate::map::{job_to_summary, run_to_summary};
use crate::page_query::{quick_search_text, ShowMode};
use crate::types::{JobSummary, RunSummary};
use crate::validate::{validate_job_id, validate_run_id};

/// Fingerprint scope for jobs table cursors.
pub const JOBS_CURSOR_SCOPE: &str = "boson.jobs";

/// Fingerprint scope for runs table cursors; includes the URL job scope.
#[must_use]
pub fn runs_cursor_scope(scope_job_id: Option<&str>) -> String {
    format!("boson.runs:{}", scope_job_id.unwrap_or_default())
}

/// Where a cursor-path page starts reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListStart<K> {
    /// Read with the keyset method strictly after this key; `None` reads from the start.
    After(Option<K>),
    /// Read with the offset method from this row.
    Offset(usize),
}

/// True when a jobs `DataTable` request can page by cursor.
///
/// Requires no quick search and only `status` filter rules, which the coordinator applies.
#[must_use]
pub fn jobs_cursor_eligible(request: &PageRequest) -> bool {
    quick_search_text(request).is_none()
        && request
            .filter
            .as_ref()
            .is_none_or(|f| f.items.iter().all(|r| r.field == "status"))
}

/// True when a runs `DataTable` request can page by cursor.
///
/// Requires no quick search, and either no filter rules or (without a URL job scope) a single
/// `job_id` equals rule, which the coordinator applies.
#[must_use]
pub fn runs_cursor_eligible(request: &PageRequest, scope_job_id: Option<&str>) -> bool {
    if quick_search_text(request).is_some() {
        return false;
    }
    let Some(filter) = request.filter.as_ref() else {
        return true;
    };
    match filter.items.as_slice() {
        [] => true,
        [rule] => {
            scope_job_id.is_none_or(str::is_empty)
                && rule.field == "job_id"
                && matches!(rule.operator.as_str(), "equals" | "is")
        }
        _ => false,
    }
}

/// Where a cursor-path jobs page starts.
#[must_use]
pub fn jobs_list_start(request: &PageRequest) -> ListStart<JobKey> {
    list_start(request, JOBS_CURSOR_SCOPE, parse_job_key)
}

/// Where a cursor-path runs page starts.
#[must_use]
pub fn runs_list_start(request: &PageRequest, scope_job_id: Option<&str>) -> ListStart<RunKey> {
    list_start(request, &runs_cursor_scope(scope_job_id), parse_run_key)
}

/// Build a jobs page from `limit + 1` rows read at `start`.
#[must_use]
pub fn jobs_page_from_rows(
    mut rows: Vec<Job>,
    request: &PageRequest,
    limit: u32,
    start: &ListStart<JobKey>,
    total_count: Option<u64>,
) -> Page<JobSummary> {
    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = next_cursor(has_more, start, request, limit, JOBS_CURSOR_SCOPE, || {
        rows.last().map(|j| {
            format!(
                "{}|{}",
                j.created_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
                j.job_id
            )
        })
    });
    Page {
        items: rows.into_iter().map(job_to_summary).collect(),
        has_more,
        total_count,
        ..Page::default()
    }
    .with_next_cursor(next_cursor)
}

/// Build a runs page from `limit + 1` rows read at `start`.
#[must_use]
pub fn runs_page_from_rows(
    mut rows: Vec<Run>,
    request: &PageRequest,
    scope_job_id: Option<&str>,
    limit: u32,
    start: &ListStart<RunKey>,
    total_count: Option<u64>,
) -> Page<RunSummary> {
    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let scope = runs_cursor_scope(scope_job_id);
    let next_cursor = next_cursor(has_more, start, request, limit, &scope, || {
        rows.last().map(|r| {
            format!(
                "{}|{}",
                r.started_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
                r.run_id
            )
        })
    });
    Page {
        items: rows.into_iter().map(run_to_summary).collect(),
        has_more,
        total_count,
        ..Page::default()
    }
    .with_next_cursor(next_cursor)
}

fn list_start<K>(request: &PageRequest, scope: &str, parse: fn(&str) -> Option<K>) -> ListStart<K> {
    let key =
        PageCursor::from_request(request, scope).and_then(|c| c.key.as_deref().and_then(parse));
    match key {
        Some(key) => ListStart::After(Some(key)),
        None if request.offset == 0 => ListStart::After(None),
        None => ListStart::Offset(request.offset as usize),
    }
}

fn next_cursor<K>(
    has_more: bool,
    start: &ListStart<K>,
    request: &PageRequest,
    limit: u32,
    scope: &str,
    last_key: impl FnOnce() -> Option<String>,
) -> Option<String> {
    if !has_more || matches!(start, ListStart::Offset(_)) {
        return None;
    }
    let position = u64::from(request.offset) + u64::from(limit);
    let fingerprint = request.query_fingerprint(scope);
    last_key().map(|key| PageCursor::with_key(position, key, fingerprint).encode())
}

fn split_key(key: &str) -> Option<(DateTime<Utc>, &str)> {
    let (at, id) = key.split_once('|')?;
    let at = DateTime::parse_from_rfc3339(at).ok()?.with_timezone(&Utc);
    Some((at, id))
}

fn parse_job_key(key: &str) -> Option<JobKey> {
    let (created_at, id) = split_key(key)?;
    validate_job_id(id).ok()?;
    Some(JobKey {
        created_at,
        id: id.to_string(),
    })
}

fn parse_run_key(key: &str) -> Option<RunKey> {
    let (started_at, id) = split_key(key)?;
    validate_run_id(id).ok()?;
    Some(RunKey {
        started_at,
        id: id.to_string(),
    })
}

/// Rows a [`scan_parts`] call reads from the coordinator per keyset read.
pub const SCAN_CHUNK: usize = 100;

/// Most rows one [`scan_parts`] call reads before it hands back a partial page.
///
/// "Only yours" over a long history of other teams' rows would otherwise read the whole
/// history for one page. A page cut short this way still carries a cursor, so the next page
/// carries on where the read stopped.
pub const MAX_SCAN_ROWS: usize = 5_000;

/// One part of a "yours first" list read by [`scan_parts`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPart {
    /// Every row (Everyone's, or Yours first when nothing is owned).
    All,
    /// Rows of task types or queues the viewer's teams own.
    Yours,
    /// Everyone else's rows, after [`ScanPart::Yours`] under Yours first.
    Others,
}

impl ScanPart {
    /// The part a list starts with. With nothing owned every mode lists everyone's rows.
    #[must_use]
    pub const fn first(mode: ShowMode, any_owned: bool) -> Self {
        match (mode, any_owned) {
            (ShowMode::Everyones, _) | (_, false) => Self::All,
            (ShowMode::YoursFirst | ShowMode::OnlyYours, true) => Self::Yours,
        }
    }

    /// The part read after this one runs out, if any.
    #[must_use]
    pub const fn next(self, mode: ShowMode) -> Option<Self> {
        match (self, mode) {
            (Self::Yours, ShowMode::YoursFirst) => Some(Self::Others),
            _ => None,
        }
    }

    /// Whether a row belongs to this part, given whether the viewer owns its resource.
    #[must_use]
    pub const fn keeps(self, owned: bool) -> bool {
        match self {
            Self::All => true,
            Self::Yours => owned,
            Self::Others => !owned,
        }
    }

    const fn tag(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Yours => Some("y"),
            Self::Others => Some("o"),
        }
    }
}

/// A place in a "yours first" list: the part, and the last row read in it (`None` = its start).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartKey<K> {
    /// Part being read.
    pub part: ScanPart,
    /// Key of the last row read in that part; `None` reads from the part's start.
    pub key: Option<K>,
}

/// Where a "yours first" page starts reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartStart<K> {
    /// Read from this place.
    At(PartKey<K>),
    /// No cursor fits this request: skip this many rows from the list's start first.
    Offset(usize),
}

/// Rows a [`scan_parts`] call kept, with where the next page starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartScan<T, K> {
    /// Kept rows in list order, at most the number asked for.
    pub rows: Vec<T>,
    /// Where the next page starts; `None` when the list has no more rows.
    pub resume: Option<PartKey<K>>,
}

/// Read a "yours first" list from `start` until `want` rows are kept, by keyset.
///
/// `read(part, after, n)` returns up to `n` rows strictly after `after` in the list's usual
/// order, with the request's filters applied. Each part reads the same ordered rows and keeps
/// those `is_owned` says belong to it ([`ScanPart::keeps`]); when the Yours part runs out
/// under Yours first, the Others part starts from the beginning. Paging stays in order across
/// pages because the next cursor names the part as well as the key. A read stops after
/// [`MAX_SCAN_ROWS`] rows and resumes from the last row read.
pub async fn scan_parts<T, K, F, Fut>(
    start: PartKey<K>,
    mode: ShowMode,
    want: usize,
    is_owned: impl Fn(&T) -> bool,
    key_of: impl Fn(&T) -> K,
    mut read: F,
) -> PartScan<T, K>
where
    K: Clone,
    F: FnMut(ScanPart, Option<K>, usize) -> Fut,
    Fut: std::future::Future<Output = Vec<T>>,
{
    if want == 0 {
        return PartScan {
            rows: Vec::new(),
            resume: Some(start),
        };
    }
    let PartKey {
        mut part,
        key: mut after,
    } = start;
    let mut kept: Vec<(ScanPart, K, T)> = Vec::new();
    let mut scanned = 0usize;
    loop {
        let batch = read(part, after.clone(), SCAN_CHUNK).await;
        let ran_out = batch.len() < SCAN_CHUNK;
        scanned += batch.len();
        for row in batch {
            let key = key_of(&row);
            after = Some(key.clone());
            if part.keeps(is_owned(&row)) {
                kept.push((part, key, row));
                if kept.len() > want {
                    break;
                }
            }
        }
        if kept.len() > want {
            kept.truncate(want);
            let resume = kept.last().map(|(p, k, _)| PartKey {
                part: *p,
                key: Some(k.clone()),
            });
            return PartScan {
                rows: kept.into_iter().map(|(_, _, r)| r).collect(),
                resume,
            };
        }
        if ran_out {
            match part.next(mode) {
                Some(next) => {
                    part = next;
                    after = None;
                }
                None => {
                    return PartScan {
                        rows: kept.into_iter().map(|(_, _, r)| r).collect(),
                        resume: None,
                    };
                }
            }
        } else if scanned >= MAX_SCAN_ROWS {
            return PartScan {
                rows: kept.into_iter().map(|(_, _, r)| r).collect(),
                resume: Some(PartKey { part, key: after }),
            };
        }
    }
}

/// Where a "yours first" jobs page starts, for `mode` and whether the viewer owns anything.
#[must_use]
pub fn jobs_part_start(
    request: &PageRequest,
    mode: ShowMode,
    any_owned: bool,
) -> PartStart<JobKey> {
    part_start(request, JOBS_CURSOR_SCOPE, mode, any_owned, parse_job_key)
}

/// Where a "yours first" runs page starts, for `mode` and whether the viewer owns anything.
#[must_use]
pub fn runs_part_start(
    request: &PageRequest,
    scope_job_id: Option<&str>,
    mode: ShowMode,
    any_owned: bool,
) -> PartStart<RunKey> {
    part_start(
        request,
        &runs_cursor_scope(scope_job_id),
        mode,
        any_owned,
        parse_run_key,
    )
}

/// Build a jobs page from a [`scan_parts`] read; the cursor names the part and key to resume at.
#[must_use]
pub fn jobs_page_from_scan(
    scan: PartScan<Job, JobKey>,
    request: &PageRequest,
    limit: u32,
    total_count: Option<u64>,
) -> Page<JobSummary> {
    let next_cursor = scan.resume.as_ref().map(|at| {
        part_cursor(request, limit, JOBS_CURSOR_SCOPE, at, |k| {
            format_key(k.created_at, &k.id)
        })
    });
    Page {
        has_more: next_cursor.is_some(),
        items: scan.rows.into_iter().map(job_to_summary).collect(),
        total_count,
        ..Page::default()
    }
    .with_next_cursor(next_cursor)
}

/// Build a runs page from a [`scan_parts`] read; the cursor names the part and key to resume at.
#[must_use]
pub fn runs_page_from_scan(
    scan: PartScan<Run, RunKey>,
    request: &PageRequest,
    scope_job_id: Option<&str>,
    limit: u32,
    total_count: Option<u64>,
) -> Page<RunSummary> {
    let scope = runs_cursor_scope(scope_job_id);
    let next_cursor = scan.resume.as_ref().map(|at| {
        part_cursor(request, limit, &scope, at, |k| {
            format_key(k.started_at, &k.id)
        })
    });
    Page {
        has_more: next_cursor.is_some(),
        items: scan.rows.into_iter().map(run_to_summary).collect(),
        total_count,
        ..Page::default()
    }
    .with_next_cursor(next_cursor)
}

fn format_key(at: DateTime<Utc>, id: &str) -> String {
    format!("{}|{id}", at.to_rfc3339_opts(SecondsFormat::Nanos, true))
}

fn part_cursor<K>(
    request: &PageRequest,
    limit: u32,
    scope: &str,
    at: &PartKey<K>,
    key: impl Fn(&K) -> String,
) -> String {
    let key = at.key.as_ref().map_or_else(String::new, key);
    let key = match at.part.tag() {
        Some(tag) => format!("{tag}:{key}"),
        None => key,
    };
    let position = u64::from(request.offset) + u64::from(limit);
    PageCursor::with_key(position, key, request.query_fingerprint(scope)).encode()
}

fn part_start<K>(
    request: &PageRequest,
    scope: &str,
    mode: ShowMode,
    any_owned: bool,
    parse: fn(&str) -> Option<K>,
) -> PartStart<K> {
    let at = PageCursor::from_request(request, scope)
        .and_then(|c| c.key)
        .and_then(|key| parse_part_key(&key, parse));
    match at {
        Some(at) => PartStart::At(at),
        None if request.offset == 0 => PartStart::At(PartKey {
            part: ScanPart::first(mode, any_owned),
            key: None,
        }),
        None => PartStart::Offset(request.offset as usize),
    }
}

fn parse_part_key<K>(raw: &str, parse: fn(&str) -> Option<K>) -> Option<PartKey<K>> {
    let (part, rest) = match raw.split_once(':') {
        Some(("y", rest)) => (ScanPart::Yours, rest),
        Some(("o", rest)) => (ScanPart::Others, rest),
        _ => (ScanPart::All, raw),
    };
    if rest.is_empty() {
        return (part != ScanPart::All).then_some(PartKey { part, key: None });
    }
    parse(rest).map(|key| PartKey {
        part,
        key: Some(key),
    })
}
