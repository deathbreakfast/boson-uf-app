use std::collections::HashMap;

use orbital_data::{DataRecord, DataValue};

use crate::components::format::{format_attempts, format_waited};
use crate::server::page_query::job_status_key;
use crate::server::{JobStatusDto, JobSummary};

/// "HH:MM" from an RFC 3339 timestamp (times keep this format until the Timestamp rollout).
pub fn format_clock(iso: &str) -> String {
    let time = iso.split('T').nth(1).unwrap_or_default();
    let time = time.split(['.', '+', 'Z']).next().unwrap_or_default();
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() >= 2 {
        format!("{}:{}", parts[0], parts[1])
    } else {
        iso.to_string()
    }
}

pub fn job_to_record(job: &JobSummary) -> DataRecord {
    DataRecord::new(
        job.job_id.clone(),
        HashMap::from([
            ("job_id".into(), DataValue::Text(job.job_id.clone())),
            ("task_name".into(), DataValue::Text(job.task_name.clone())),
            (
                "status".into(),
                DataValue::Text(job_status_key(job.status).into()),
            ),
            ("pool".into(), DataValue::Text(job.pool.clone())),
            ("args".into(), DataValue::Text(job.args_summary.clone())),
            (
                "waited".into(),
                DataValue::Text(format_waited(job.waited_ms)),
            ),
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
                DataValue::Text(
                    job.failed_at
                        .as_deref()
                        .map_or_else(String::new, format_clock),
                ),
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
