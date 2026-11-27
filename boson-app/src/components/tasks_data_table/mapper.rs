use std::collections::HashMap;

use orbital_data::{DataRecord, DataValue};

use crate::server::TaskSummary;

/// Owner cell text when the host supplies no owner for a task type.
pub const NO_OWNER: &str = "Not set yet";

pub fn task_to_record(task: TaskSummary) -> DataRecord {
    let id = task.name.clone();
    let success_rate = task
        .success_rate_pct
        .map_or_else(|| "No runs yet".to_string(), |r| format!("{r:.1}%"));
    let owner = task.owner.unwrap_or_else(|| NO_OWNER.to_string());

    DataRecord::new(
        id,
        HashMap::from([
            ("name".into(), DataValue::Text(task.name)),
            ("owner".into(), DataValue::Text(owner)),
            ("queue".into(), DataValue::Text(task.effective_pool)),
            (
                "jobs_queued".into(),
                DataValue::Text(task.jobs_queued.to_string()),
            ),
            (
                "runs_total".into(),
                DataValue::Text(task.runs_total.to_string()),
            ),
            ("success_rate".into(), DataValue::Text(success_rate)),
        ]),
    )
}

pub fn record_to_task_name(record: &DataRecord) -> String {
    record
        .get("name")
        .map_or_else(String::new, orbital_data::DataValue::display_string)
}
