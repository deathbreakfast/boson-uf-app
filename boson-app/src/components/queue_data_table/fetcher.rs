use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use boson_backend::JobsTab;
use leptos::prelude::*;
use orbital::primitives::PageFetcher;
use orbital_data::DataRecord;
use orbital_paging::{Page, PageRequest};

use crate::server::{list_jobs_datatable_page, JobSummary, JOBS_PAGE_SIZE};

use super::mapper::job_to_record;

pub const QUEUE_TABLE_PAGE_SIZE: u32 = JOBS_PAGE_SIZE;

/// Rows the jobs tables have loaded, by job id, so dialogs can say what a job is.
pub type JobRows = RwSignal<HashMap<String, JobSummary>>;

/// Fetch one tab's page and remember its rows in `rows`.
pub fn build_jobs_fetcher(tab: JobsTab, rows: JobRows) -> PageFetcher {
    Arc::new(move |request: PageRequest| {
        Box::pin(async move {
            let page = list_jobs_datatable_page(request, Some(tab.key().to_string())).await?;
            rows.update(|map| {
                for job in &page.items {
                    map.insert(job.job_id.clone(), job.clone());
                }
            });
            Ok(Page {
                items: page.items.iter().map(job_to_record).collect(),
                has_more: page.has_more,
                total_count: page.total_count,
                next_request_offset: page.next_request_offset,
                next_cursor: page.next_cursor,
            })
        }) as Pin<Box<dyn Future<Output = Result<Page<DataRecord>, ServerFnError>> + Send>>
    })
}
