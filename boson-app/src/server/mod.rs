//! Leptos server functions and DTOs for Boson UI.
//!
//! DTOs and pure mapping helpers live in [`boson_backend`] so contracts stay
//! unit/integration-testable without the host UI graph. Server functions run on
//! SSR only and use [`higgs::Higgs::from_request()`] plus [`helpers::require_session`]
//! on every endpoint. Job actions `cancel_job`, `retry_jobs` and `drop_jobs` go through one
//! access check (`BosonAdmin` today). Mutator `update_task_config`, task-config
//! reads (`get_task_config`, `list_gluon_pools_for_boson_task_config`), and run
//! history lists (`list_runs_page`, `list_runs_datatable_page`) require Gauge
//! permission `BosonAdmin`. Task-config endpoints additionally mirror the UI
//! email-verification gate via [`helpers::require_email_verified`]. Run history
//! (`list_runs_page`, `list_runs_datatable_page`, `get_run`) also requires
//! `BosonAdmin`. Dashboard reads that expose pool stats or run errors
//! (`get_needs_attention`, `get_failing_most`) require `BosonAdmin` too. Queue
//! endpoints (`list_queues`, `get_queue`, `pause_queue`, `resume_queue`,
//! `purge_queue`) check `BosonAdmin` through one access helper in `queues.rs`.
//!
//! The UI uses paginated list endpoints (`get_tasks_page`, `list_jobs_page`,
//! `list_runs_page`) and single-record getters (`get_task`, `get_run`, etc.).
//!
//! ## Errors
//!
//! Fallible ops return [`ServerFnError`](leptos::prelude::ServerFnError) (Leptos
//! boundary). Stable message prefixes integrators can match:
//!
//! - Auth: `Authentication is required…`, `Email verification is required…`
//! - Context: `Boson backend not in request context`
//! - Id validation: [`boson_backend::BosonIdError`] Display text
//! - Range: `Invalid range_secs:…` ([`boson_backend::BosonInputError`])
//! - Config update: `Invalid task config update:…` (including a pool the host's
//!   [`boson_backend::BosonPoolProvider`] does not offer)
//! - Config load: `Failed to load task config:…` / `Task config not found:…`
//! - Mutators: `Failed to cancel job:…` / `Failed to update config:…`; `retry_jobs` and
//!   `drop_jobs` report per-job failures in `JobActionOutcome::failed`
//! - Queues: `Queue not found:…`, `Failed to pause queue:…` / `Failed to resume queue:…` /
//!   `Failed to purge queue:…`, and `Queues need an embedded Boson runtime on this host`
//!   when the coordinator is remote
//!
//! Blank / oversized / path-unsafe ids are rejected by `boson_backend::validate_*`
//! before coordinator IO. Detail hrefs use `boson_backend::boson_*_path` helpers so
//! Orbital `paths::*` format strings cannot smuggle extra path segments. Failures
//! are traced once at the server-fn boundary (`operation`, `error_class`, safe ids).

mod dashboard;
mod gluon_pools;
mod helpers;
mod jobs;
pub mod page_query;
mod queues;
mod runs;
mod tasks;
mod types;

pub use types::*;

/// Permission name required for Boson admin mutators / task-config reads
/// (manifest: [`crate::permissions::BosonPermission::BosonAdmin`]).
pub const BOSON_ADMIN_PERMISSION: &str = "BosonAdmin";

pub use dashboard::{
    get_dashboard_stats, get_failing_most, get_needs_attention, get_run_stats_series,
};
pub use gluon_pools::list_gluon_pools_for_boson_task_config;
pub use jobs::{
    cancel_job, drop_jobs, list_jobs_datatable_page, list_jobs_page, retry_jobs, JobAction,
};
pub use queues::{get_queue, list_queues, pause_queue, purge_queue, resume_queue};
pub use runs::{
    get_run, get_run_detail, list_runs_datatable_page, list_runs_page, list_task_names,
};
pub use tasks::{
    get_task, get_task_active_jobs, get_task_config, get_task_detail, get_tasks,
    get_tasks_datatable_page, get_tasks_page, update_task_config,
};
