mod columns;
mod fetcher;
mod mapper;

use std::collections::HashSet;

use boson_backend::{JobsTab, ShowMode};
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use orbital::components::{EmptyState, MessageBar, MessageBarIntent};
use orbital::primitives::{
    DataTable, DataTableEmptyView, DataTableEvents, DataTableFeatures, DataTableHeaderChromeConfig,
    DataTableLoadingView, DataTableLocale, DataTableNoResultsView, DataTableSource,
    DataTableToolbarConfig, PagingMode,
};

use crate::components::{
    track_load_error, BosonDataTableShell, BosonDataTableSkeletonRows, LoadError, TableLoadError,
};

pub use columns::{failed_table_columns, FAILED_TABLE_COLUMNS};
use columns::{queue_table_columns, JOBS_TABLE_COLUMNS};
pub use fetcher::{build_jobs_fetcher, JobRows, QUEUE_TABLE_PAGE_SIZE};

/// Jobs DataTable for the Active or Finished tab, with search, a status filter and Cancel.
#[component]
pub fn JobsDataTable(
    /// Which tab the table lists (Active or Finished).
    tab: JobsTab,
    /// Rows loaded so far, by job id; the fetcher fills it.
    rows: JobRows,
    /// Ids of jobs with a cancel in flight.
    cancel_pending: ReadSignal<HashSet<String>>,
    /// Called with a job id when its Cancel button is pressed.
    on_cancel: Callback<String>,
    /// Bump to reload the rows in place.
    #[prop(optional)]
    refresh_signal: Option<Signal<u32>>,
    /// Whose jobs come first.
    #[prop(optional)]
    show: ShowMode,
) -> impl IntoView {
    let navigate = use_navigate();

    let on_row_click = Callback::new(move |(id,): (String,)| {
        navigate(
            &boson_backend::boson_runs_job_filter_path(&id),
            NavigateOptions::default(),
        );
    });

    let load_error: LoadError = RwSignal::new(None);
    let retry = RwSignal::new(0u32);
    let on_retry = Callback::new(move |()| retry.update(|n| *n += 1));
    let refresh = Signal::derive(move || {
        refresh_signal
            .map_or(0, |s| s.get())
            .wrapping_add(retry.get())
    });
    let data_source = DataTableSource::Server {
        fetcher: track_load_error(build_jobs_fetcher(tab, rows, show), load_error),
        page_size: QUEUE_TABLE_PAGE_SIZE,
    };

    let (empty_message, empty_description) = match tab {
        JobsTab::Finished => (
            "No finished jobs",
            "Jobs that succeed or are canceled show up here.",
        ),
        _ => (
            "Nothing waiting",
            "Every job has been picked up. Finished jobs are under Finished.",
        ),
    };

    let locale = DataTableLocale {
        quick_search_placeholder: "Search by job id".to_string(),
        ..DataTableLocale::default()
    };

    view! {
        <BosonDataTableShell data_testid="boson-jobs-data-table">
            <DataTable
                data_source=data_source
                paging=PagingMode::Paged
                flex=true
                features=DataTableFeatures::MULTI_FILTER
                columns=queue_table_columns(cancel_pending, on_cancel)
                sortable=false
                locale=locale
                refresh_signal=refresh
                toolbar_config=DataTableToolbarConfig {
                    quick_search: true,
                    // P7: filter panel motion deferred — DataTableFilterPanel owns Popover
                    // internally; track upstream orbital-datagrid filter/Popover motion hook.
                    filter_panel: true,
                    column_picker: false,
                    pivot: false,
                    export_menu: false,
                }
                header_chrome=DataTableHeaderChromeConfig {
                    column_menu: false,
                    column_filter_button: false,
                    column_hide: false,
                    show_table_grid: true,
                }
                events=DataTableEvents {
                    on_row_click: Some(on_row_click),
                    ..Default::default()
                }
            >
                <DataTableLoadingView slot>
                    <BosonDataTableSkeletonRows columns=JOBS_TABLE_COLUMNS />
                </DataTableLoadingView>
                <DataTableEmptyView slot>
                    <TableLoadError error=load_error noun="jobs" on_retry=on_retry>
                        <EmptyState message=empty_message description=empty_description />
                    </TableLoadError>
                </DataTableEmptyView>
                <DataTableNoResultsView slot>
                    <TableLoadError error=load_error noun="jobs" on_retry=on_retry>
                        <MessageBar intent=MessageBarIntent::Info>
                            "No jobs match your search or filters."
                        </MessageBar>
                    </TableLoadError>
                </DataTableNoResultsView>
            </DataTable>
        </BosonDataTableShell>
    }
}
