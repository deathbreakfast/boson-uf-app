use std::sync::Arc;

use leptos::prelude::*;
use orbital::primitives::{DataTableColumnDef, Link};
use orbital_data::DataRecord;

use crate::components::{success_rate_help, AccessBadge, BosonHelpColumnHeader};

use super::mapper::record_to_task_name;

pub fn tasks_table_columns() -> Vec<DataTableColumnDef> {
    let name_view = Arc::new(|record: DataRecord| {
        let name = record_to_task_name(&record);
        let testid = format!("task-{name}");
        let href = boson_backend::boson_task_path(&name);
        view! {
            <div data-testid=testid>
                <Link href=href>{name}</Link>
            </div>
        }
        .into_any()
    });

    let access_view = Arc::new(|record: DataRecord| {
        let access = record
            .get("access")
            .map_or_else(String::new, orbital_data::DataValue::display_string);
        view! { <AccessBadge access=access /> }.into_any()
    });

    let success_header = Arc::new(|| {
        view! {
            <BosonHelpColumnHeader label="Success rate" info=success_rate_help() />
        }
        .into_any()
    });

    vec![
        DataTableColumnDef::new("name", "Task")
            .with_sortable(false)
            .with_cell_view(name_view),
        DataTableColumnDef::new("owner", "Owner").with_sortable(false),
        DataTableColumnDef::new("queue", "Queue").with_sortable(false),
        DataTableColumnDef::new("jobs_queued", "Waiting").with_sortable(false),
        DataTableColumnDef::new("runs_total", "Runs").with_sortable(false),
        DataTableColumnDef::new("success_rate", "Success rate")
            .with_sortable(false)
            .with_header_view(success_header),
        DataTableColumnDef::new("access", "Your access")
            .with_sortable(false)
            .with_cell_view(access_view),
    ]
}
