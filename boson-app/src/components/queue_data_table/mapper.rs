use std::collections::HashMap;

use orbital_data::{DataRecord, DataValue};

use crate::components::format::format_attempts;
use crate::components::owner_fields;
use crate::server::page_query::job_status_key;
use crate::server::{JobStatusDto, JobSummary};

/// Record field that is `"yes"` when the reader holds Run on the job's task type.
pub const CAN_RUN_FIELD: &str = "can_run";

pub fn job_to_record(job: &JobSummary) -> DataRecord {
    let [owner, yours] = owner_fields(job.owner.as_deref(), job.yours);
    DataRecord::new(
        job.job_id.clone(),
        HashMap::from([
            owner,
            yours,
            ("job_id".into(), DataValue::Text(job.job_id.clone())),
            ("task_name".into(), DataValue::Text(job.task_name.clone())),
            (
                "status".into(),
                DataValue::Text(job_status_key(job.status).into()),
            ),
            ("pool".into(), DataValue::Text(job.pool.clone())),
            ("args".into(), DataValue::Text(job.args_summary.clone())),
            // Waited reads "6 min ago" from the enqueue time, through Timestamp.
            ("waited".into(), DataValue::Text(job.created_at.clone())),
            (
                "attempts".into(),
                DataValue::Text(format_attempts(job.attempt, job.max_attempts)),
            ),
            (
                "last_error".into(),
                DataValue::Text(job.last_error.clone().unwrap_or_default()),
            ),
            (
                "failed_at".into(),
                DataValue::Text(job.failed_at.clone().unwrap_or_default()),
            ),
            (
                CAN_RUN_FIELD.into(),
                DataValue::Text(if job.can_run { "yes" } else { "" }.into()),
            ),
        ]),
    )
}

pub fn job_status_from_key(key: &str) -> JobStatusDto {
    match key {
        "running" => JobStatusDto::Running,
        "success" => JobStatusDto::Success,
        "failed" => JobStatusDto::Failed,
        "canceled" => JobStatusDto::Canceled,
        _ => JobStatusDto::Queued,
    }
}
