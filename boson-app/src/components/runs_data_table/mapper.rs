use std::collections::HashMap;

use orbital_data::{DataRecord, DataValue};

use crate::components::format::{format_attempts, format_seconds};
use crate::components::owner_fields;
use crate::server::page_query::run_status_key;
use crate::server::{RunStatusDto, RunSummary};

pub fn run_to_record(run: RunSummary) -> DataRecord {
    let id = run.run_id.clone();
    let [owner, yours] = owner_fields(run.owner.as_deref(), run.yours);
    DataRecord::new(
        id,
        HashMap::from([
            owner,
            yours,
            ("run_id".into(), DataValue::Text(run.run_id)),
            ("job_id".into(), DataValue::Text(run.job_id)),
            ("task_name".into(), DataValue::Text(run.task_name)),
            (
                "status".into(),
                DataValue::Text(run_status_key(run.status).into()),
            ),
            (
                "attempt".into(),
                DataValue::Text(format_attempts(run.attempt, run.max_attempts)),
            ),
            ("started_at".into(), DataValue::Text(run.started_at)),
            (
                "duration_ms".into(),
                DataValue::Text(format_seconds(run.duration_ms)),
            ),
        ]),
    )
}

pub fn run_status_from_key(key: &str) -> RunStatusDto {
    match key {
        "success" => RunStatusDto::Success,
        "failed" => RunStatusDto::Failed,
        "canceled" => RunStatusDto::Canceled,
        "timeout" => RunStatusDto::Timeout,
        _ => RunStatusDto::Running,
    }
}
