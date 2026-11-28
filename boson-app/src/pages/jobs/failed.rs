//! The Failed tab: jobs that used every attempt, with Retry and Drop for one or many.

use std::collections::HashSet;

use boson_backend::{JobsTab, ShowMode, FAILED_JOB_RETENTION_DAYS};
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use orbital::components::{
    Body1Strong, Caption1, Card, EmptyState, MessageBar, MessageBarIntent, SpacingSize,
};
use orbital::primitives::{
    Button, ButtonAppearance, DataTable, DataTableEmptyView, DataTableEvents, DataTableFeatures,
    DataTableHeaderChromeConfig, DataTableLoadingView, DataTableLocale, DataTableNoResultsView,
    DataTableSelectionMode, DataTableSource, DataTableToolbarConfig, Flex, FlexAlign, FlexWrap,
    PagingMode,
};

use crate::components::{
    build_jobs_fetcher, failed_table_columns, track_load_error, BosonCardContent,
    BosonDataTableShell, BosonDataTableSkeletonRows, JobRows, LoadError, TableLoadError,
    FAILED_TABLE_COLUMNS, QUEUE_TABLE_PAGE_SIZE,
};
use crate::errors::friendly_error;
use crate::server::{drop_jobs, retry_jobs, JobSummary};

use super::copy::{action_failures, drop_success, retry_success};
use super::drop_dialog::DropJobsDialog;
use super::retry_dialog::RetryJobsDialog;

/// Signals a Retry or Drop updates as it runs.
#[derive(Clone, Copy)]
struct ActionState {
    targets: RwSignal<Vec<String>>,
    pending: RwSignal<bool>,
    success: RwSignal<Option<String>>,
    error: RwSignal<Option<String>>,
    selected: RwSignal<HashSet<String>>,
    refresh: RwSignal<u32>,
}

#[derive(Clone, Copy)]
enum JobBulkAction {
    Retry,
    Drop,
}

/// Run the action on the targets, report what happened, then close the dialog, clear the
/// selection and reload the rows.
fn confirm_action(
    state: ActionState,
    action: JobBulkAction,
    dialog: RwSignal<bool>,
) -> Callback<()> {
    Callback::new(move |()| {
        let ids = state.targets.get_untracked();
        state.pending.set(true);
        state.success.set(None);
        state.error.set(None);
        spawn_local_scoped(async move {
            let (result, verb) = match action {
                JobBulkAction::Retry => (retry_jobs(ids).await, "retry"),
                JobBulkAction::Drop => (drop_jobs(ids).await, "drop"),
            };
            match result {
                Ok(outcome) => {
                    state.success.set(match action {
                        JobBulkAction::Retry => retry_success(&outcome),
                        JobBulkAction::Drop => drop_success(&outcome),
                    });
                    state.error.set(action_failures(verb, &outcome));
                }
                Err(e) => state.error.set(Some(friendly_error(&e).sentence())),
            }
            dialog.set(false);
            state.pending.set(false);
            state.selected.set(HashSet::new());
            state.refresh.update(|n| *n += 1);
        });
    })
}

/// Failed jobs table with multi-select, the bulk bar, and the Retry and Drop dialogs.
#[component]
pub fn FailedJobsPanel(
    /// Rows loaded so far, by job id.
    rows: JobRows,
    /// Bump to reload the rows in place.
    refresh: RwSignal<u32>,
    /// Card class from the page layout.
    card_class: String,
    /// Card content class from the page layout.
    card_content_class: String,
    /// Whose failed jobs come first.
    #[prop(optional)]
    show: ShowMode,
) -> impl IntoView {
    let selected = RwSignal::new(HashSet::<String>::new());
    let targets = RwSignal::new(Vec::<String>::new());
    let retry_open = RwSignal::new(false);
    let drop_open = RwSignal::new(false);
    let pending = RwSignal::new(false);
    let success = RwSignal::new(None::<String>);
    let error = RwSignal::new(None::<String>);

    let target_jobs = Signal::derive(move || {
        let ids = targets.get();
        rows.with(|map| {
            ids.iter()
                .filter_map(|id| map.get(id).cloned())
                .collect::<Vec<JobSummary>>()
        })
    });

    let open_for = move |ids: Vec<String>, dialog: RwSignal<bool>| {
        targets.set(ids);
        dialog.set(true);
    };
    let selected_ids = move || {
        let mut ids: Vec<String> = selected.get_untracked().into_iter().collect();
        ids.sort();
        ids
    };

    let on_retry_row = Callback::new(move |id: String| open_for(vec![id], retry_open));
    let on_drop_row = Callback::new(move |id: String| open_for(vec![id], drop_open));

    let action = ActionState {
        targets,
        pending,
        success,
        error,
        selected,
        refresh,
    };
    let on_confirm_retry = confirm_action(action, JobBulkAction::Retry, retry_open);
    let on_confirm_drop = confirm_action(action, JobBulkAction::Drop, drop_open);

    view! {
        <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
            {move || success.get().map(|message| view! {
                <div data-testid="failed-jobs-success">
                    <MessageBar intent=MessageBarIntent::Success>{message}</MessageBar>
                </div>
            })}
            {move || error.get().map(|message| view! {
                <div data-testid="failed-jobs-error">
                    <MessageBar intent=MessageBarIntent::Error>{message}</MessageBar>
                </div>
            })}
            <FailedBulkBar
                selected=selected
                on_retry=Callback::new(move |()| open_for(selected_ids(), retry_open))
                on_drop=Callback::new(move |()| open_for(selected_ids(), drop_open))
            />
            <FailedJobsTable
                rows=rows
                refresh=refresh
                selected=selected
                on_retry_row=on_retry_row
                on_drop_row=on_drop_row
                card_class=card_class
                card_content_class=card_content_class
                show=show
            />
            <Caption1>
                {format!(
                    "Failed jobs are kept {FAILED_JOB_RETENTION_DAYS} days, then dropped. Their runs stay in Runs."
                )}
            </Caption1>
            <RetryJobsDialog
                open=retry_open
                jobs=target_jobs
                pending=pending
                on_confirm=on_confirm_retry
            />
            <DropJobsDialog
                open=drop_open
                jobs=target_jobs
                pending=pending
                on_confirm=on_confirm_drop
            />
        </Flex>
    }
}

/// "N selected · Retry N · Drop N · Clear selection", shown while rows are selected.
#[component]
fn FailedBulkBar(
    selected: RwSignal<HashSet<String>>,
    on_retry: Callback<()>,
    on_drop: Callback<()>,
) -> impl IntoView {
    view! {
        {move || {
            let n = selected.with(HashSet::len);
            (n > 0).then(|| view! {
                <div data-testid="failed-jobs-bulk-bar">
                    <Flex align=FlexAlign::Center wrap=FlexWrap::Wrap gap=SpacingSize::Size120.flex_gap()>
                        <Body1Strong>{format!("{n} selected")}</Body1Strong>
                        <span data-testid="failed-bulk-retry">
                            <Button
                                appearance=ButtonAppearance::Primary
                                on_click=Callback::new(move |_| on_retry.run(()))
                            >
                                {format!("Retry {n}")}
                            </Button>
                        </span>
                        <span data-testid="failed-bulk-drop">
                            <Button
                                appearance=ButtonAppearance::Secondary
                                on_click=Callback::new(move |_| on_drop.run(()))
                            >
                                {format!("Drop {n}")}
                            </Button>
                        </span>
                        <Button
                            appearance=ButtonAppearance::Subtle
                            on_click=Callback::new(move |_| selected.set(HashSet::new()))
                        >
                            "Clear selection"
                        </Button>
                    </Flex>
                </div>
            })
        }}
    }
}

/// The failed jobs table: multi-select, with Retry and Drop on each row.
#[component]
fn FailedJobsTable(
    rows: JobRows,
    refresh: RwSignal<u32>,
    selected: RwSignal<HashSet<String>>,
    on_retry_row: Callback<String>,
    on_drop_row: Callback<String>,
    card_class: String,
    card_content_class: String,
    show: ShowMode,
) -> impl IntoView {
    let load_error: LoadError = RwSignal::new(None);
    let on_retry = Callback::new(move |()| refresh.update(|n| *n += 1));
    let data_source = DataTableSource::Server {
        fetcher: track_load_error(build_jobs_fetcher(JobsTab::Failed, rows, show), load_error),
        page_size: QUEUE_TABLE_PAGE_SIZE,
    };

    let locale = DataTableLocale {
        quick_search_placeholder: "Search by job id".to_string(),
        ..DataTableLocale::default()
    };

    view! {
        <Card class=card_class>
            <BosonCardContent class=card_content_class>
                <BosonDataTableShell data_testid="boson-failed-jobs-data-table">
                    <DataTable
                        data_source=data_source
                        paging=PagingMode::Paged
                        flex=true
                        features=DataTableFeatures::empty()
                        columns=failed_table_columns(on_retry_row, on_drop_row)
                        sortable=false
                        locale=locale
                        selection_mode=DataTableSelectionMode::Multiselect
                        selection=Signal::derive(move || Some(selected.get()))
                        refresh_signal=Signal::derive(move || refresh.get())
                        toolbar_config=DataTableToolbarConfig {
                            quick_search: true,
                            filter_panel: false,
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
                            on_selection_change: Some(Callback::new(move |ids: HashSet<String>| {
                                if selected.with_untracked(|s| *s != ids) {
                                    selected.set(ids);
                                }
                            })),
                            ..Default::default()
                        }
                    >
                        <DataTableLoadingView slot>
                            <BosonDataTableSkeletonRows columns=FAILED_TABLE_COLUMNS />
                        </DataTableLoadingView>
                        <DataTableEmptyView slot>
                            <TableLoadError error=load_error noun="failed jobs" on_retry=on_retry>
                                <EmptyState
                                    message="No failed jobs"
                                    description="When a job uses every attempt it waits here for Retry or Drop."
                                />
                            </TableLoadError>
                        </DataTableEmptyView>
                        <DataTableNoResultsView slot>
                            <TableLoadError error=load_error noun="failed jobs" on_retry=on_retry>
                                <MessageBar intent=MessageBarIntent::Info>
                                    "No failed jobs match your search."
                                </MessageBar>
                            </TableLoadError>
                        </DataTableNoResultsView>
                    </DataTable>
                </BosonDataTableShell>
            </BosonCardContent>
        </Card>
    }
}
