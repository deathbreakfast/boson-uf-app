//! Shared Boson UI building blocks (tables, badges, help, motion).
//!
//! Pages under [`crate::pages`] compose these with Orbital layouts. Prefer crate-root
//! rustdoc for operator workflows; this module is the inventory of reusable pieces.

mod back_link;
mod card_surface;
mod data_table_loading;
pub mod format;
mod help;
mod job_status_badge;
mod motion;
mod queue_data_table;
mod run_status_badge;
mod runs_data_table;
mod stat_card;
mod table_link;
mod task_summary_panel;
mod tasks_data_table;

pub use back_link::BosonBackLink;
pub use card_surface::{boson_table_page_layout, BosonCardContent, BosonDataTableShell};
// Re-export layout class bundle for hosts / pages that type the return of
// `boson_table_page_layout` explicitly (not always named in-crate).
#[allow(unused_imports)]
pub use card_surface::BosonTablePageClasses;
pub use data_table_loading::{BosonDataTableRefetchSkeleton, BosonDataTableSkeletonRows};
pub use help::{
    attempt_help, duration_help, pool_help, run_outcomes_chart_help, runs_24h_help,
    success_rate_help, tasks_overview_help, BosonHelpCardHeader, BosonHelpColumnHeader,
};
pub use job_status_badge::JobStatusBadge;
pub use motion::{boson_error_reveal_motion, boson_kpi_enter_motion};
pub use queue_data_table::{
    build_jobs_fetcher, failed_table_columns, JobRows, JobsDataTable, FAILED_TABLE_COLUMNS,
    QUEUE_TABLE_PAGE_SIZE,
};
pub use run_status_badge::RunStatusBadge;
pub use runs_data_table::{short_id_link, RunsDataTable, RunsFilters, RunsTableScope};
pub use stat_card::BosonHelpStatCard;
pub use table_link::{boson_table_link_styles, BosonTableLink, BosonTruncatedTableCellLink};
pub use task_summary_panel::TaskSummaryPanel;
pub use tasks_data_table::{TaskCardActions, TasksDataTable};
