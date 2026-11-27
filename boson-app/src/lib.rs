#![recursion_limit = "256"]
//! Boson operations app — monitor background work queues, task config, and run history.
//!
//! Leptos UI mounted under `/boson` so operators can see queued jobs, edit task
//! configuration, and inspect run attempts without building custom pages. Registers
//! alongside other product apps via `uf_app!` and requires an authenticated session with
//! `BosonAdmin` before server functions load coordinator data.
//!
//! Orbital inventory macros (`uf_app!`, `orbital_routes_extract`) emit undocumented
//! associated items, so this crate allows `missing_docs` at the crate root while keeping
//! hand-written modules and items documented.
//!
//! ## Features
//!
//! - **Boson admin routes** — Provides the nested `/boson` route tree behind auth for
//!   dashboard, tasks, jobs, and runs. Mount once when the host router starts.
//!   [Get started](#mount-boson-routes)
//! - **Dashboard KPIs** — Shows task, queue, and run counters on [`BosonRootPage`] via
//!   [`get_dashboard_stats`] plus run-trend charts from [`get_run_stats_series`].
//!   [Get started](#dashboard-kpis)
//! - **Tasks browser** — Lists registered tasks and supports detail and config edits via
//!   [`get_tasks`], [`get_task`], and [`update_task_config`].
//!   [Get started](#browse-tasks)
//! - **Jobs** — Lists waiting and running jobs and lets operators cancel them, and lists
//!   failed jobs to retry or drop, via [`list_jobs_datatable_page`], [`cancel_job`],
//!   [`retry_jobs`] and [`drop_jobs`]. [Get started](#inspect-jobs)
//! - **Runs browser** — Lists run attempts by status, task and time, searches by id, and
//!   opens detail pages via [`list_runs_datatable_page`] and [`get_run`].
//!   [Get started](#browse-runs)
//! - **Server function wrappers** — Exposes [`mod@server`] Higgs `#[server]` fns and DTO
//!   re-exports backed by [`boson_backend`] pure mapping helpers.
//!
//! ## Mount Boson routes
//!
//! [`BosonRoutes`] nests the full `/boson` subtree inside a host Leptos `<Routes>` tree.
//! Operators get visibility into task configuration, queued jobs, and run history.
//! Mount during host router setup at startup, alongside other `uf_app!` product routes —
//! the macro registers launcher metadata and the `/boson` inventory entry.
//!
//! **Prerequisites:** `ssr` on this crate; authenticated session; `BosonAdmin` permission
//! ([`BOSON_ADMIN_PERMISSION`]); Boson backend in Leptos request context for IO.
//!
//! ```rust,ignore
//! use leptos::prelude::*;
//! use leptos_router::components::Routes;
//! use boson_app::BosonRoutes;
//!
//! view! {
//!     <Routes fallback=|| "not found">
//!         <BosonRoutes />
//!     </Routes>
//! }
//! ```
//!
//! On success `/boson` resolves to the dashboard, `/boson/tasks` lists registered tasks,
//! and nested jobs and run routes load their pages. Unauthenticated sessions are rejected
//! by server functions — see root `SECURITY.md`.
//!
//! ## Dashboard KPIs
//!
//! The dashboard answers how much background work is active right now: registered task
//! count, queued and running jobs, and runs started in the last 24 hours.
//! [`BosonRootPage`] calls [`get_dashboard_stats`] on each SSR render and
//! [`get_run_stats_series`] for trend charts — use this landing page after mounting routes
//! when operators need a quick health snapshot.
//!
//! **Prerequisites:** [`BosonRoutes`] mounted; `ssr` feature; `BosonAdmin` permission;
//! Boson backend request context wired.
//!
//! ```rust,ignore
//! use boson_app::{
//!     BosonRootPage, get_dashboard_stats, get_run_stats_series, DashboardStats,
//! };
//! use leptos::prelude::*;
//!
//! // BosonRootPage calls these on each SSR render:
//! view! { <BosonRootPage /> };
//! let stats: DashboardStats = get_dashboard_stats().await?;
//! assert_eq!(stats.task_count, 3);
//! assert_eq!(stats.jobs_queued, 5);
//!
//! let series = get_run_stats_series(86_400).await?;
//! assert!(!series.is_empty());
//! ```
//!
//! On success `stats` carries `task_count`, `jobs_queued`, `jobs_running`, and `runs_today`;
//! `series` holds chart buckets for successful and failed runs. `range_secs` must be
//! `86_400` (24h) or `604_800` (7d); other values return `Invalid range_secs:…`.
//! Blank or unsafe path ids are rejected by `boson_backend::validate_*` before
//! coordinator IO.
//!
//! ## Browse tasks
//!
//! Task pages list registered handlers with effective pool and priority overlays.
//! [`BosonTasksIndexPage`] loads [`get_tasks`] for the index; [`BosonTaskDetailPage`] calls
//! [`get_task`] for one task name; [`BosonTaskConfigPage`] uses [`update_task_config`] after
//! email verification. Open these routes when operators adjust retry policy or pool assignment.
//!
//! **Prerequisites:** Routes mounted; task names must pass `boson_backend::validate_task_name`;
//! config edits require a verified email.
//!
//! ```rust,ignore
//! use boson_app::{
//!     BosonTasksIndexPage, get_tasks, get_task, update_task_config, TaskSummary,
//! };
//! use boson_backend::UpdateTaskConfigRequest;
//! use leptos::prelude::*;
//!
//! // BosonTasksIndexPage loads get_tasks for the index:
//! view! { <BosonTasksIndexPage /> };
//! let tasks: Vec<TaskSummary> = get_tasks().await?;
//! assert_eq!(tasks.first().map(|t| t.name.as_str()), Some("orders.task"));
//!
//! let detail = get_task("orders.task".into()).await?;
//! assert_eq!(detail.name, "orders.task");
//!
//! update_task_config(
//!     "orders.task".into(),
//!     UpdateTaskConfigRequest { priority: Some(10), ..Default::default() },
//! )
//! .await?;
//! ```
//!
//! On success the index returns sorted [`TaskSummary`] rows and detail resolves one task
//! or maps a missing name to a server error. Config updates persist through the coordinator
//! after validation. Task list/detail fail closed when task config cannot be loaded
//! (`Failed to load task config:…`) instead of substituting silent defaults.
//!
//! ## Inspect jobs
//!
//! [`BosonJobsPage`] opens on Active: the queued and running jobs, the ones someone can act
//! on. Failed lists jobs that used every attempt, and Finished the ones that succeeded or
//! were canceled. Each page reads [`list_jobs_datatable_page`] with the tab's key. Cancel
//! asks first and then names the canceled job; on Failed, [`retry_jobs`] starts jobs again
//! from attempt 1 as the person who pressed Retry, and [`drop_jobs`] takes them off the list.
//! Use this page when draining a backlog, stopping a runaway enqueue or clearing dead
//! letters. `/boson/queue` redirects here.
//!
//! **Prerequisites:** Routes mounted; job ids must pass `boson_backend::validate_job_id`.
//!
//! ```rust,ignore
//! use boson_app::{retry_jobs, cancel_job, BosonJobsPage};
//! use leptos::prelude::*;
//!
//! view! { <BosonJobsPage /> };
//! cancel_job("job-1".into()).await?;
//! let outcome = retry_jobs(vec!["job-2".into()]).await?;
//! assert_eq!(outcome.retried[0].from_job_id, "job-2");
//! ```
//!
//! On success cancel moves a job to canceled, and each retried job comes back as a new
//! queued job in [`JobActionOutcome::retried`]. Jobs an action can't touch (not failed,
//! unknown) are listed in [`JobActionOutcome::failed`] while the rest go ahead.
//!
//! ## Browse runs
//!
//! Run pages list execution attempts and full error detail on the detail view.
//! [`BosonRunsIndexPage`] loads [`list_runs_datatable_page`] with status, task and time
//! filters from the URL (`?status=&task=&from=&to=`) and pages by keyset cursor at any
//! history size;
//! [`BosonRunDetailPage`] calls [`get_run`] for one run id. Open these routes when operators
//! audit failures or trace a job back to its attempts.
//!
//! **Prerequisites:** Routes mounted; run ids must pass `boson_backend::validate_run_id`.
//!
//! ```rust,ignore
//! use boson_app::{BosonRunsIndexPage, list_runs_page, get_run, RunSummary};
//! use leptos::prelude::*;
//!
//! // BosonRunsIndexPage loads list_runs_page with optional filters:
//! view! { <BosonRunsIndexPage /> };
//! let page = list_runs_page(0, 20, None).await?;
//! let first: &RunSummary = page.items.first().expect("run row");
//! assert_eq!(first.run_id, "run-1");
//!
//! let detail = get_run("run-1".into()).await?;
//! assert_eq!(detail.run_id, "run-1");
//! ```
//!
//! On success the index returns [`RunSummary`] preview rows and detail resolves one attempt
//! or errors when the id is unknown. Oversized or path-unsafe ids fail validation before
//! coordinator lookup.
//!
//! ## Feature flags
//!
//! | Flag | Effect |
//! |------|--------|
//! | `ssr` | Server-side Leptos split; required for `#[server]` fns and Boson IO. |
//! | `hydrate` | Client-side hydration for routed pages and Orbital shell components. |
//!
//! ## Routes
//!
//! Mounted under `/boson` by [`BosonRoutes`]. Task config requires a verified email.
//!
//! | Path | Page | Key server fn(s) |
//! |---|---|---|
//! | `/boson` | [`BosonRootPage`] | [`get_dashboard_stats`], [`get_run_stats_series`] |
//! | `/boson/tasks` | [`BosonTasksIndexPage`] | [`get_tasks_page`], [`get_tasks_datatable_page`] |
//! | `/boson/tasks/:task_name` | [`BosonTaskDetailPage`] | [`get_task`] |
//! | `/boson/tasks/:task_name/config` (email-verified) | [`BosonTaskConfigPage`] | [`get_task_config`], [`update_task_config`], [`list_gluon_pools_for_boson_task_config`] |
//! | `/boson/jobs` (`?tab=active`, `failed`, `finished`) | [`BosonJobsPage`] | [`list_jobs_datatable_page`], [`cancel_job`], [`retry_jobs`], [`drop_jobs`] |
//! | `/boson/queue` | [`BosonQueueRedirect`] | redirects to `/boson/jobs` |
//! | `/boson/runs` | [`BosonRunsIndexPage`] | [`list_runs_datatable_page`], [`list_task_names`] |
//! | `/boson/runs/:id` | [`BosonRunDetailPage`] | [`get_run`] |
//!
//! ## Examples
//!
//! Start with [Mount Boson routes](#mount-boson-routes). The `boson-backend` unit and integ
//! suites in `docs/VERIFICATION.md` cover server-fn contracts. Runnable host:
//! `examples/protected-boson-host` (auth + dashboard KPIs; inventory `boson` / `/boson`).
//!
//! ## Where to look next
//!
//! - [`BosonLayout`] — shared app bar / nav shell wrapping every route.
//! - [`mod@server`] — server functions and DTOs backing the UI.
//! - [`mod@pages`] — route page components (dashboard, tasks, jobs, runs).
//! - `components` — shared Orbital-composed UI pieces (tables, badges, help).
//! - [`permissions::BosonPermission`] — permission manifest for `BosonAdmin`.
//! - [`live`] / [`photon_ws`] — client poll-tick and SSR route merge for live updates.
//! - `boson_backend` — id validation and pure mapping helpers used by these server fns.
//! - Workspace `docs/VERIFICATION.md` — Layer 1 unit/integ and Layer 2 Playwright CI gates.

#![allow(missing_docs)]
// Orbital / Leptos macros leave cfg-gated items that look unused under `ssr` alone;
// keep the allow narrow to unknown_lints (workspace also allows unknown_lints).
#![cfg_attr(feature = "ssr", allow(unknown_lints))]
use leptos::prelude::*;
use leptos_router::{
    components::{ParentRoute, Route},
    path, Lazy,
};
use uf_product_macros::uf_app;

mod components;
/// Lab-only overrides for `boson-uf-app-e2e` (setters require feature `e2e-lab`).
pub mod e2e_lab;
mod layout;
mod lazy_routes;
/// Client-side live-update hooks (poll tick, placeholder broadcast sources).
pub mod live;
pub mod pages;
/// Permission manifest for Boson admin server functions.
pub mod permissions;
#[cfg(feature = "ssr")]
pub mod photon_ws;
/// SSR server functions and DTOs backing the Boson UI.
pub mod server;

pub use layout::BosonLayout;
pub use lazy_routes::{
    prefetch_family, BosonJobsRoute, BosonLayoutRouteView, BosonRootRoute, BosonRunDetailRoute,
    BosonRunsIndexRoute, BosonTaskDetailRoute, BosonTasksIndexRoute, BosonVerifiedTaskConfigRoute,
};
pub use pages::{
    BosonJobsPage, BosonQueueRedirect, BosonRootPage, BosonRunDetailPage, BosonRunsIndexPage,
    BosonTaskConfigPage, BosonTaskDetailPage, BosonTasksIndexPage,
};
pub use server::{
    cancel_job, drop_jobs, get_dashboard_stats, get_run, get_run_stats_series, get_task,
    get_task_config, get_tasks, get_tasks_datatable_page, get_tasks_page,
    list_gluon_pools_for_boson_task_config, list_jobs_datatable_page, list_jobs_page,
    list_runs_datatable_page, list_runs_page, list_task_names, retry_jobs, update_task_config,
    BosonPoolProvider, DashboardStats, GluonPoolPickRow, JobActionOutcome, JobSummary, RunSummary,
    TaskConfigDto, TaskSummary, BOSON_ADMIN_PERMISSION,
};

uf_app! {
    name: "Boson",
    id: "boson",
    description: "Background work management",
    icon: "⚛️",
    version: "0.1.0",
    routes: BosonRoutes,
    route_path: "/boson",
    repository: "https://github.com/unified-field-dev/boson-uf-app",
    permission_manifest: permissions::BosonPermission,
}

/// Boson's nested route tree, gated behind an auth guard and mounted at `/boson`.
///
/// Leaf pages are [`LazyRoute`](leptos_router::LazyRoute) views so
/// `cargo leptos --split` can emit a separate WASM chunk for this family.
/// Registers dashboard, task, jobs, and run routes. The task config route additionally
/// requires a verified email. Intended to be used inside a host `<Routes>` component, e.g.
/// `<BosonRoutes />`.
#[allow(missing_docs)]
#[orbital_macros::orbital_routes_extract]
#[component(transparent)]
pub fn BosonRoutes() -> impl leptos_router::MatchNestedRoutes + Clone {
    view! {
        <ParentRoute path=path!("boson") view=BosonLayoutRouteView>
            <Route path=path!("") view={Lazy::<BosonRootRoute>::new()} />
            <Route path=path!("tasks") view={Lazy::<BosonTasksIndexRoute>::new()} />
            <Route path=path!("tasks/:task_name") view={Lazy::<BosonTaskDetailRoute>::new()} />
            <Route path=path!("tasks/:task_name/config") view={Lazy::<BosonVerifiedTaskConfigRoute>::new()} />
            <Route path=path!("jobs") view={Lazy::<BosonJobsRoute>::new()} />
            <Route path=path!("queue") view=BosonQueueRedirect />
            <Route path=path!("runs") view={Lazy::<BosonRunsIndexRoute>::new()} />
            <Route path=path!("runs/:id") view={Lazy::<BosonRunDetailRoute>::new()} />
        </ParentRoute>
    }
    .into_inner()
}
