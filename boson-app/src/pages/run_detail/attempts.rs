use leptos::prelude::*;
use orbital::components::{Card, Step, StepStatus, Stepper, Subtitle2};
use orbital::primitives::CardHeader;

use crate::components::BosonCardContent;
use crate::server::{RunStatusDto, RunSummary};

/// Longest error text an attempt step shows before cutting it off.
const ATTEMPT_ERROR_MAX_CHARS: usize = 120;

const fn step_status(status: RunStatusDto) -> StepStatus {
    match status {
        RunStatusDto::Running => StepStatus::Active,
        RunStatusDto::Success => StepStatus::Done,
        RunStatusDto::Failed | RunStatusDto::Timeout => StepStatus::Failed,
        RunStatusDto::Canceled => StepStatus::Skipped,
    }
}

/// "HH:MM" from an RFC3339 timestamp.
fn clock(rfc3339: &str) -> &str {
    rfc3339.get(11..16).unwrap_or(rfc3339)
}

/// The step's message: the first line of the error (cut short) and the duration.
fn step_message(run: &RunSummary) -> Option<String> {
    let error = run
        .error_message
        .as_deref()
        .and_then(|e| e.lines().next())
        .map(|line| {
            if line.chars().count() > ATTEMPT_ERROR_MAX_CHARS {
                let cut: String = line.chars().take(ATTEMPT_ERROR_MAX_CHARS).collect();
                format!("{cut}…")
            } else {
                line.to_string()
            }
        });
    let duration = run.duration_ms.map(boson_backend::format_duration_ms);
    match (error, duration) {
        (Some(e), Some(d)) => Some(format!("{e} · {d}")),
        (Some(e), None) => Some(e),
        (None, Some(d)) => Some(d),
        (None, None) => None,
    }
}

/// Every run of the job as a Stepper, oldest first, with time, error summary and duration.
#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn RunAttempts(
    /// Runs of the job, oldest first.
    attempts: Vec<RunSummary>,
    /// The run this page shows (marked in its label).
    current_run_id: String,
) -> impl IntoView {
    let steps: Vec<Step> = attempts
        .iter()
        .map(|run| {
            let mut label = format!("Attempt {} · {}", run.attempt, clock(&run.started_at));
            if run.run_id == current_run_id {
                label.push_str(" (this run)");
            }
            Step {
                label,
                status: Signal::stored(step_status(run.status)),
                message: step_message(run).into(),
            }
        })
        .collect();

    view! {
        <Card>
            <CardHeader>
                <Subtitle2>"Attempts"</Subtitle2>
            </CardHeader>
            <BosonCardContent>
                <div data-testid="run-attempts">
                    <Stepper steps=steps />
                </div>
            </BosonCardContent>
        </Card>
    }
}
