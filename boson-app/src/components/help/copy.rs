//! Plain-language help copy for Boson operator UI.

use leptos::prelude::*;
use orbital::components::Caption1;

fn caption(text: &'static str) -> AnyView {
    view! { <Caption1>{text}</Caption1> }.into_any()
}

pub fn signature_help() -> AnyView {
    caption(
        "JSON shape of arguments registered with #[boson::task]. Used to match payloads when jobs are enqueued.",
    )
}

pub fn defaults_help() -> AnyView {
    caption("Pool and priority from task registration before any per-task UI override is applied.")
}

pub fn effective_help() -> AnyView {
    caption(
        "Resolved routing after per-task configuration. Differs from defaults when you override pool or priority on the config page.",
    )
}

pub fn failed_24h_help() -> AnyView {
    caption("Runs that failed in the last 24 hours, across every task type.")
}

pub fn run_outcomes_chart_help() -> AnyView {
    caption(
        "Runs by outcome in each time bucket: four hours wide for 24h, a day wide for 7d. \
         Succeeded, Failed, and Timed out or canceled stack in each bar. \
         Pick a bar to open the runs that started in it.",
    )
}

pub fn failing_most_help() -> AnyView {
    caption(
        "Task types with the most failed runs in the last 24 hours. \
         Success rate counts that day's runs, with one decimal.",
    )
}

pub fn pool_help() -> AnyView {
    caption(
        "Virtual pool routes this job to Gluon worker capacity. \"global\" is the default when no pool override is configured.",
    )
}

pub fn attempt_help() -> AnyView {
    caption(
        "One-based retry index for this run within the job. Attempt 1 is the first try; higher values indicate retries.",
    )
}

pub fn duration_help() -> AnyView {
    caption(
        "Wall-clock time in milliseconds from start to finish. Running jobs show \"-\" until they complete.",
    )
}

pub fn success_rate_help() -> AnyView {
    caption(
        "Percentage of completed runs that succeeded for this task. \"-\" when no runs have finished yet.",
    )
}
