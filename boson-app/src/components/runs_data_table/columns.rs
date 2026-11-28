use std::sync::Arc;

use leptos::prelude::*;
use orbital::primitives::{ColumnType, DataTableColumnDef, Link, TimestampStyle, Tooltip};
use orbital_data::DataRecord;

use super::mapper::run_status_from_key;
use crate::components::format::short_id;
use crate::components::{
    attempt_help, duration_help, owner_column, timestamp_column, BosonHelpColumnHeader,
    RunStatusBadge,
};

/// Columns the runs table shows; the skeleton rows match this count.
pub const RUNS_TABLE_COLUMNS: usize = 8;

fn text(record: &DataRecord, key: &str) -> String {
    record
        .get(key)
        .map_or_else(String::new, orbital_data::DataValue::display_string)
}

/// A short id that links to `href`, with the full id in its tooltip.
pub fn short_id_link(id: String, href: String, testid: String) -> AnyView {
    let label = short_id(&id);
    view! {
        <div data-testid=testid attr:data-skip-row-click="">
            <Tooltip content=id>
                <Link href=href>{label}</Link>
            </Tooltip>
        </div>
    }
    .into_any()
}

pub fn runs_table_columns() -> Vec<DataTableColumnDef> {
    let status_view = Arc::new(|record: DataRecord| {
        view! { <RunStatusBadge status=run_status_from_key(&text(&record, "status")) /> }.into_any()
    });

    let run_id_view = Arc::new(|record: DataRecord| {
        let run_id = text(&record, "run_id");
        let href = boson_backend::boson_run_path(&run_id);
        let testid = format!("runs-row-{run_id}");
        short_id_link(run_id, href, testid)
    });

    let job_id_view = Arc::new(|record: DataRecord| {
        let job_id = text(&record, "job_id");
        let href = boson_backend::boson_runs_job_filter_path(&job_id);
        let testid = format!("runs-job-{job_id}");
        short_id_link(job_id, href, testid)
    });

    let task_view = Arc::new(|record: DataRecord| {
        let task = text(&record, "task_name");
        let href = boson_backend::boson_task_path(&task);
        view! {
            <div attr:data-skip-row-click="">
                <Link href=href>{task}</Link>
            </div>
        }
        .into_any()
    });

    let attempt_header = Arc::new(|| {
        view! {
            <BosonHelpColumnHeader label="Attempt" info=attempt_help() />
        }
        .into_any()
    });

    let duration_header = Arc::new(|| {
        view! {
            <BosonHelpColumnHeader label="Duration" info=duration_help() />
        }
        .into_any()
    });

    vec![
        DataTableColumnDef::new("run_id", "Run")
            .with_sortable(false)
            .with_cell_view(run_id_view),
        DataTableColumnDef::new("task_name", "Task")
            .with_sortable(false)
            .with_cell_view(task_view),
        owner_column(),
        DataTableColumnDef::new("status", "Status")
            .with_col_type(ColumnType::SingleSelect)
            .with_sortable(false)
            .with_cell_view(status_view),
        DataTableColumnDef::new("attempt", "Attempt")
            .with_sortable(false)
            .with_header_view(attempt_header),
        timestamp_column("started_at", "Started", TimestampStyle::DateTime),
        DataTableColumnDef::new("duration_ms", "Duration")
            .with_sortable(false)
            .with_header_view(duration_header),
        DataTableColumnDef::new("job_id", "Job")
            .with_sortable(false)
            .with_cell_view(job_id_view),
    ]
}
