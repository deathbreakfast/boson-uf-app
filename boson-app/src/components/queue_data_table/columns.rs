use std::collections::HashSet;
use std::sync::Arc;

use leptos::prelude::*;
use orbital::primitives::{
    Body1, Button, ButtonAppearance, ButtonSize, ColumnType, DataTableColumnDef, Flex, Link,
};
use orbital_data::DataRecord;

use crate::components::{pool_help, short_id_link, BosonHelpColumnHeader, JobStatusBadge};
use crate::server::JobStatusDto;

use super::mapper::job_status_from_key;

/// Columns the Active and Finished tables show; the skeleton rows match this count.
pub const JOBS_TABLE_COLUMNS: usize = 7;
/// Columns the Failed table shows, besides the selection checkbox.
pub const FAILED_TABLE_COLUMNS: usize = 7;

fn text(record: &DataRecord, key: &str) -> String {
    record
        .get(key)
        .map_or_else(String::new, orbital_data::DataValue::display_string)
}

/// Path of a queue's page.
pub fn queue_path(pool: &str) -> String {
    format!(
        "/boson/queues/{}",
        boson_backend::encode_ops_path_segment(pool)
    )
}

fn job_column() -> DataTableColumnDef {
    let view = Arc::new(|record: DataRecord| {
        let job_id = text(&record, "job_id");
        let href = boson_backend::boson_runs_job_filter_path(&job_id);
        let testid = format!("job-card-{job_id}");
        short_id_link(job_id, href, testid)
    });
    DataTableColumnDef::new("job_id", "Job")
        .with_sortable(false)
        .with_cell_view(view)
}

fn task_column() -> DataTableColumnDef {
    let view = Arc::new(|record: DataRecord| {
        let task = text(&record, "task_name");
        let href = boson_backend::boson_task_path(&task);
        view! {
            <div attr:data-skip-row-click="">
                <Link href=href>{task}</Link>
            </div>
        }
        .into_any()
    });
    DataTableColumnDef::new("task_name", "Task")
        .with_sortable(false)
        .with_cell_view(view)
}

fn args_column() -> DataTableColumnDef {
    let view =
        Arc::new(|record: DataRecord| view! { <Body1>{text(&record, "args")}</Body1> }.into_any());
    DataTableColumnDef::new("args", "Arguments")
        .with_sortable(false)
        .with_filterable(false)
        .with_cell_view(view)
}

/// Columns for the Active and Finished tabs.
pub fn queue_table_columns(
    cancel_pending: ReadSignal<HashSet<String>>,
    on_cancel: Callback<String>,
) -> Vec<DataTableColumnDef> {
    let status_view = Arc::new(move |record: DataRecord| {
        view! { <JobStatusBadge status=job_status_from_key(&text(&record, "status")) /> }.into_any()
    });

    let pool_view = Arc::new(|record: DataRecord| {
        let pool = text(&record, "pool");
        let href = queue_path(&pool);
        view! {
            <div attr:data-skip-row-click="">
                <Link href=href>{pool}</Link>
            </div>
        }
        .into_any()
    });

    let pool_header = Arc::new(|| {
        view! {
            <BosonHelpColumnHeader label="Queue" info=pool_help() />
        }
        .into_any()
    });

    let cancel_view = Arc::new(move |record: DataRecord| {
        let job_id = text(&record, "job_id");
        let status = job_status_from_key(&text(&record, "status"));
        let can_cancel = status == JobStatusDto::Queued || status == JobStatusDto::Running;

        if !can_cancel {
            return ().into_any();
        }

        let job_id_cancel = job_id.clone();
        let job_id_check = job_id.clone();
        let job_id_label = job_id.clone();

        view! {
            <div data-testid=format!("job-cancel-{job_id}") attr:data-skip-row-click="">
                <Button
                    size=ButtonSize::Small
                    appearance=ButtonAppearance::Subtle
                    disabled=Signal::derive(move || cancel_pending.get().contains(&job_id_check))
                    on_click=Callback::new(move |_| on_cancel.run(job_id_cancel.clone()))
                >
                    {move || {
                        if cancel_pending.get().contains(&job_id_label) {
                            "Cancelling…"
                        } else {
                            "Cancel"
                        }
                    }}
                </Button>
            </div>
        }
        .into_any()
    });

    vec![
        job_column(),
        task_column(),
        args_column(),
        DataTableColumnDef::new("status", "Status")
            .with_col_type(ColumnType::SingleSelect)
            .with_sortable(false)
            .with_cell_view(status_view),
        DataTableColumnDef::new("pool", "Queue")
            .with_sortable(false)
            .with_header_view(pool_header)
            .with_cell_view(pool_view),
        DataTableColumnDef::new("waited", "Waited")
            .with_sortable(false)
            .with_filterable(false),
        DataTableColumnDef::new("cancel", "Action")
            .with_sortable(false)
            .with_filterable(false)
            .with_cell_view(cancel_view),
    ]
}

/// Columns for the Failed tab, with Retry and Drop on each row.
pub fn failed_table_columns(
    on_retry: Callback<String>,
    on_drop: Callback<String>,
) -> Vec<DataTableColumnDef> {
    let actions_view = Arc::new(move |record: DataRecord| {
        let job_id = text(&record, "job_id");
        let retry_id = job_id.clone();
        let drop_id = job_id.clone();
        view! {
            <div attr:data-skip-row-click="">
                <Flex>
                    <span data-testid=format!("failed-retry-{job_id}")>
                        <Button
                            size=ButtonSize::Small
                            appearance=ButtonAppearance::Subtle
                            on_click=Callback::new(move |_| on_retry.run(retry_id.clone()))
                        >
                            "Retry"
                        </Button>
                    </span>
                    <span data-testid=format!("failed-drop-{job_id}")>
                        <Button
                            size=ButtonSize::Small
                            appearance=ButtonAppearance::Subtle
                            on_click=Callback::new(move |_| on_drop.run(drop_id.clone()))
                        >
                            "Drop"
                        </Button>
                    </span>
                </Flex>
            </div>
        }
        .into_any()
    });

    vec![
        job_column(),
        task_column(),
        args_column(),
        DataTableColumnDef::new("attempts", "Attempts")
            .with_sortable(false)
            .with_filterable(false),
        DataTableColumnDef::new("last_error", "Last error")
            .with_sortable(false)
            .with_filterable(false),
        DataTableColumnDef::new("failed_at", "Failed")
            .with_sortable(false)
            .with_filterable(false),
        DataTableColumnDef::new("actions", "Actions")
            .with_sortable(false)
            .with_filterable(false)
            .with_cell_view(actions_view),
    ]
}
