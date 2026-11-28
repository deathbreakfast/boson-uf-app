//! Leptos server functions and DTOs for Boson UI.
//!
//! DTOs and pure mapping helpers live in [`boson_backend`] so contracts stay
//! unit/integration-testable without the host UI graph. Server functions run on
//! SSR only and use [`higgs::Higgs::from_request()`] plus [`helpers::require_session`]
//! on every endpoint.
//!
//! ## Access
//!
//! Access is per task type and per queue, checked as the session actor through
//! `uf_product::access::require` (see [`crate::permissions`]):
//!
//! - Task type: View for its numbers, runs, run errors and job arguments (`get_task_detail`,
//!   `get_run`, `get_run_detail`, `get_task_active_jobs`, the runs and jobs lists); Run for
//!   `cancel_job`, `retry_jobs` and `drop_jobs` (checked per job; a refused job lands in
//!   `JobActionOutcome::failed`); Edit for `get_task_config` and `update_task_config`.
//! - Queue: View for `get_queue`, Edit for `pause_queue` and `resume_queue`, Delete for
//!   `purge_queue`.
//! - The lists (`get_tasks*`, `list_queues`) show every resource with the reader's role in
//!   `access`, and no numbers where the reader can't View.
//! - The dashboard's cross-type panels (`get_needs_attention`, `get_failing_most`) still need
//!   `BosonAdmin`.
//!
//! Task-config endpoints also mirror the UI email-verification gate via
//! [`helpers::require_email_verified`].
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
//! - Access: `permission_denied::{kind}.{id}.{action}` from `uf_product::access::require`
//!   (read it back with [`crate::permissions::denied_resource`])
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

#[cfg(feature = "ssr")]
mod access;
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
