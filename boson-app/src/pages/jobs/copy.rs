//! Sentences the Jobs page and its dialogs show about jobs.

use crate::components::format::{count_noun, short_id};
use crate::server::{JobActionOutcome, JobStatusDto, JobSummary};

/// "media.thumbnail for asset 7781 · sizes [3]", or just the task when it has no arguments.
pub fn job_label(job: &JobSummary) -> String {
    if job.args_summary.is_empty() {
        job.task_name.clone()
    } else {
        format!("{} for {}", job.task_name, job.args_summary)
    }
}

/// "(media.thumbnail, asset 7781)" for success messages.
fn job_parenthetical(job: &JobSummary) -> String {
    if job.args_summary.is_empty() {
        format!("({})", job.task_name)
    } else {
        format!("({}, {})", job.task_name, job.args_summary)
    }
}

/// What the Cancel dialog says about a job.
pub fn cancel_body(job: Option<&JobSummary>) -> String {
    match job {
        Some(job) if job.status == JobStatusDto::Running => format!(
            "{} is running on {}. Cancel stops it now, and it won't retry.",
            job_label(job),
            job.pool
        ),
        Some(job) => format!(
            "{} is waiting on {}. Cancel stops it now, so it won't run or retry.",
            job_label(job),
            job.pool
        ),
        None => "Cancel stops this job now, and it won't retry.".to_string(),
    }
}

/// The message after a cancel lands, naming the job.
pub fn cancel_success(job_id: &str, job: Option<&JobSummary>) -> String {
    match job {
        Some(job) if job.status == JobStatusDto::Running => format!(
            "Canceled job {} {}. {} picks up the next job now.",
            short_id(job_id),
            job_parenthetical(job),
            job.pool
        ),
        Some(job) => format!(
            "Canceled job {} {}. It won't run.",
            short_id(job_id),
            job_parenthetical(job)
        ),
        None => format!("Canceled job {}.", short_id(job_id)),
    }
}

fn single_task(jobs: &[JobSummary]) -> Option<&JobSummary> {
    let first = jobs.first()?;
    jobs.iter()
        .all(|j| j.task_name == first.task_name)
        .then_some(first)
}

/// "up to 3 attempts" when every job's task type gives the same most attempts.
fn attempts_clause(jobs: &[JobSummary]) -> String {
    let first = jobs.first().and_then(|j| j.max_attempts);
    match first {
        Some(max) if jobs.iter().all(|j| j.max_attempts == Some(max)) => {
            format!(
                " (up to {})",
                count_noun(u64::from(max), "attempt", "attempts")
            )
        }
        _ => String::new(),
    }
}

/// Retry dialog title.
pub fn retry_title(n: usize) -> String {
    if n == 1 {
        "Retry this failed job?".to_string()
    } else {
        format!("Retry {n} failed jobs?")
    }
}

/// What the Retry dialog says: from attempt 1, today's policy, and that it runs as you.
pub fn retry_body(jobs: &[JobSummary]) -> String {
    let attempts = attempts_clause(jobs);
    match (jobs.len(), single_task(jobs)) {
        (1, Some(job)) => format!(
            "Boson starts {} again from attempt 1, with the same arguments and today's retry policy{attempts}. It runs as you.",
            job_label(job)
        ),
        (n, Some(job)) => format!(
            "Boson starts these {n} {} jobs again from attempt 1, with the same arguments and today's retry policy{attempts}. They run as you.",
            job.task_name
        ),
        (n, None) => format!(
            "Boson starts these {n} jobs again from attempt 1, each with its own arguments and its task type's retry policy today. They run as you."
        ),
    }
}

/// A warning when every job failed with the same error, so a retry may fail the same way.
pub fn retry_warning(jobs: &[JobSummary]) -> Option<String> {
    let first = jobs.first()?.last_error.clone()?;
    if !jobs
        .iter()
        .all(|j| j.last_error.as_deref() == Some(first.as_str()))
    {
        return None;
    }
    Some(match jobs.len() {
        1 => format!(
            "The last attempt failed with “{first}”. If that hasn't been fixed, it will fail the same way."
        ),
        2 => format!(
            "Both failed with “{first}”. If that hasn't been fixed, they'll fail the same way."
        ),
        n => format!(
            "All {n} failed with “{first}”. If that hasn't been fixed, they'll fail the same way."
        ),
    })
}

/// Drop dialog title.
pub fn drop_title(n: usize) -> String {
    if n == 1 {
        "Drop this failed job?".to_string()
    } else {
        format!("Drop {n} failed jobs?")
    }
}

/// What the Drop dialog says: gone from the failed list for good, runs kept.
pub fn drop_body(jobs: &[JobSummary]) -> String {
    match jobs {
        [job] => format!(
            "{} leaves the failed list and won't run again. Its runs stay in Runs.",
            job_label(job)
        ),
        jobs => format!(
            "These {} jobs leave the failed list and won't run again. Their runs stay in Runs.",
            jobs.len()
        ),
    }
}

fn join_names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Success message after Retry: how many and which queues they're waiting in.
pub fn retry_success(outcome: &JobActionOutcome) -> Option<String> {
    let n = outcome.retried.len();
    if n == 0 {
        return None;
    }
    let mut pools: Vec<String> = Vec::new();
    for job in &outcome.retried {
        if !job.pool.is_empty() && !pools.contains(&job.pool) {
            pools.push(job.pool.clone());
        }
    }
    let (waiting_who, follow_whom) = if n == 1 {
        ("It's", "it")
    } else {
        ("They're", "them")
    };
    let waiting = if pools.is_empty() {
        format!("{waiting_who} waiting")
    } else {
        format!("{waiting_who} waiting in {}", join_names(&pools))
    };
    Some(format!(
        "Retried {}. {waiting}; follow {follow_whom} under Active.",
        count_noun(n as u64, "job", "jobs")
    ))
}

/// Success message after Drop.
pub fn drop_success(outcome: &JobActionOutcome) -> Option<String> {
    let n = outcome.dropped.len();
    if n == 0 {
        return None;
    }
    let their = if n == 1 { "Its" } else { "Their" };
    Some(format!(
        "Dropped {}. {their} runs stay in Runs.",
        count_noun(n as u64, "job", "jobs")
    ))
}

/// Error message for the jobs an action couldn't act on.
pub fn action_failures(verb: &str, outcome: &JobActionOutcome) -> Option<String> {
    if outcome.failed.is_empty() {
        return None;
    }
    Some(
        outcome
            .failed
            .iter()
            .map(|f| format!("Couldn't {verb} job {}: {}.", short_id(&f.job_id), f.error))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::RetriedJob;

    fn job(id: &str, task: &str, args: &str, error: Option<&str>) -> JobSummary {
        JobSummary {
            job_id: id.into(),
            task_name: task.into(),
            status: JobStatusDto::Failed,
            priority: 0,
            pool: "global".into(),
            created_at: String::new(),
            waited_ms: 0,
            args_summary: args.into(),
            attempt: 3,
            max_attempts: Some(3),
            last_error: error.map(str::to_string),
            failed_at: None,
            can_run: true,
            owner: None,
            yours: false,
        }
    }

    #[test]
    fn retry_copy_names_task_and_attempts() {
        let jobs = vec![
            job("a", "orders.sync_invoices", "batch 1", Some("401")),
            job("b", "orders.sync_invoices", "batch 2", Some("401")),
        ];
        assert_eq!(retry_title(2), "Retry 2 failed jobs?");
        assert!(retry_body(&jobs).starts_with("Boson starts these 2 orders.sync_invoices jobs"));
        assert!(retry_body(&jobs).contains("(up to 3 attempts)"));
        assert!(retry_warning(&jobs)
            .unwrap()
            .starts_with("Both failed with “401”"));
        let mixed = vec![job("a", "t", "", Some("x")), job("b", "t", "", Some("y"))];
        assert_eq!(retry_warning(&mixed), None);
    }

    #[test]
    fn success_messages_say_where() {
        let outcome = JobActionOutcome {
            retried: vec![
                RetriedJob {
                    from_job_id: "a".into(),
                    job_id: "n1".into(),
                    pool: "global".into(),
                },
                RetriedJob {
                    from_job_id: "b".into(),
                    job_id: "n2".into(),
                    pool: "global".into(),
                },
            ],
            ..JobActionOutcome::default()
        };
        assert_eq!(
            retry_success(&outcome).unwrap(),
            "Retried 2 jobs. They're waiting in global; follow them under Active."
        );
        let dropped = JobActionOutcome {
            dropped: vec!["a".into()],
            ..JobActionOutcome::default()
        };
        assert_eq!(
            drop_success(&dropped).unwrap(),
            "Dropped 1 job. Its runs stay in Runs."
        );
        let j = job(
            "00000000-0000-4000-8000-000000000101",
            "media.thumbnail",
            "asset 7781",
            None,
        );
        assert_eq!(
            cancel_success(&j.job_id, Some(&j)),
            "Canceled job …0101 (media.thumbnail, asset 7781). It won't run."
        );
    }
}
