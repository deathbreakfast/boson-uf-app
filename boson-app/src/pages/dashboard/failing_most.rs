use std::sync::Arc;

use leptos::prelude::*;
use orbital::components::{Card, EmptyState, SpacingSize};
use orbital::primitives::{
    DataRecord, DataTable, DataTableColumnDef, DataTableEmptyView, DataTableFeatures,
    DataTableLoadingView, DataTableSource, Flex, FlexAlign, FlexJustify, Link, MessageBar,
    MessageBarIntent, PagingMode,
};

use crate::components::{
    failing_most_help, no_toolbar, row_model, success_rate_help, text_cell, BosonCardContent,
    BosonDataTableRefetchSkeleton, BosonHelpCardHeader, BosonHelpColumnHeader,
};
use crate::server::FailingTaskRow;

fn columns() -> Vec<DataTableColumnDef> {
    let task_view = Arc::new(|record: DataRecord| {
        let task = text_cell(&record, "task");
        let href = boson_backend::boson_task_path(&task);
        let testid = format!("failing-most-row-{task}");
        view! {
            <Link href=href attr:data-testid=testid>{task}</Link>
        }
        .into_any()
    });
    let rate_header = Arc::new(|| {
        view! { <BosonHelpColumnHeader label="Success rate" info=success_rate_help() /> }.into_any()
    });
    vec![
        DataTableColumnDef::new("task", "Task")
            .with_sortable(false)
            .with_cell_view(task_view),
        DataTableColumnDef::new("owner", "Owner").with_sortable(false),
        DataTableColumnDef::new("failed", "Failed (24h)").with_sortable(false),
        DataTableColumnDef::new("runs", "Runs (24h)").with_sortable(false),
        DataTableColumnDef::new("rate", "Success rate")
            .with_sortable(false)
            .with_header_view(rate_header),
        DataTableColumnDef::new("error", "Most common error").with_sortable(false),
    ]
}

fn rows(tasks: &[FailingTaskRow]) -> Vec<orbital::primitives::DataTableRowModel> {
    tasks
        .iter()
        .map(|t| {
            row_model(
                &t.task,
                &[
                    ("task", t.task.clone()),
                    ("owner", t.owner.clone()),
                    ("failed", t.failed_24h.to_string()),
                    ("runs", t.runs_24h.to_string()),
                    ("rate", t.success_rate.clone()),
                    ("error", t.most_common_error.clone()),
                ],
            )
        })
        .collect()
}

#[component]
fn FailingMostTable(
    /// Rows to show; empty shows the empty state.
    tasks: Vec<FailingTaskRow>,
    /// True while the first load is pending.
    loading: bool,
) -> impl IntoView {
    let items = RwSignal::new(rows(&tasks));
    let loading = RwSignal::new(loading);
    view! {
        <DataTable
            data_source=DataTableSource::Client(items)
            paging=PagingMode::None
            features=DataTableFeatures::empty()
            toolbar_config=no_toolbar()
            sortable=false
            columns=columns()
            loading=loading
        >
            <DataTableEmptyView slot>
                <EmptyState
                    message="No task types"
                    description="Register tasks with #[boson::task] to see them here."
                />
            </DataTableEmptyView>
            <DataTableLoadingView slot>
                <BosonDataTableRefetchSkeleton />
            </DataTableLoadingView>
        </DataTable>
    }
}

/// Failing most: task types with the most failed runs in the last 24 hours.
#[component]
pub fn FailingMostCard(
    /// Resource that loads the rows.
    rows_res: Resource<Result<Vec<FailingTaskRow>, ServerFnError>>,
) -> impl IntoView {
    view! {
        <div data-testid="dashboard-failing-most">
            <Card>
                <BosonHelpCardHeader title="Failing most" info=failing_most_help() />
                <BosonCardContent>
                    <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                        <Transition fallback=move || view! { <FailingMostTable tasks=Vec::new() loading=true /> }>
                            {move || rows_res.get().map(|r| match r {
                                Ok(tasks) => view! { <FailingMostTable tasks=tasks loading=false /> }.into_any(),
                                Err(e) => view! {
                                    <MessageBar intent=MessageBarIntent::Error>
                                        "Couldn't load task failures. " {e.to_string()}
                                    </MessageBar>
                                }.into_any(),
                            })}
                        </Transition>
                        <Flex justify=FlexJustify::End align=FlexAlign::Center>
                            <Link href=crate::paths::TASKS attr:data-testid="dashboard-failing-most-all-tasks">
                                "All tasks"
                            </Link>
                        </Flex>
                    </Flex>
                </BosonCardContent>
            </Card>
        </div>
    }
}
