mod columns;
mod fetcher;
mod mapper;

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use orbital::components::{Caption1, EmptyState, MessageBar, MessageBarIntent, SpacingSize};
use orbital::primitives::{
    use_data_table_table_state, Button, ButtonAppearance, DataTable, DataTableEmptyView,
    DataTableEvents, DataTableFeatures, DataTableFilter, DataTableFooterSlot,
    DataTableHeaderChromeConfig, DataTableLoadingView, DataTableLocale, DataTableNoResultsView,
    DataTablePaginationBar, DataTableQuickSearch, DataTableSource, DataTableTableState,
    DataTableToolbarSlot, DatePicker, DatetimeTimezone, FilterLogic, FilterOperator, FilterRule,
    Flex, FlexAlign, FlexJustify, FlexWrap, OrbitalDateTime, PagingMode, Select,
};
use orbital_data::DataValue;

use crate::components::format::count_noun;
use crate::components::{BosonDataTableShell, BosonDataTableSkeletonRows};

pub use columns::short_id_link;
use columns::{runs_table_columns, RUNS_TABLE_COLUMNS};
use fetcher::{build_runs_fetcher, RUNS_TABLE_PAGE_SIZE};

/// Scope for the Boson runs `DataTable`.
#[derive(Clone, PartialEq, Eq)]
pub enum RunsTableScope {
    All,
    ForJob(String),
}

/// Time Select value for picked dates.
const PICK_DATES: &str = "pick";

/// Toolbar filters on the Runs page, kept in the URL as `?status=&task=&from=&to=`.
///
/// `from` is a time token (`1h`, `24h`, `7d`, `30d`, `any`) or a `YYYY-MM-DD` date, and `to`
/// a date; the server resolves them (`boson_backend::parse_time_range`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunsFilters {
    /// Run status key, or empty for any status.
    pub status: String,
    /// Task name, or empty for any task.
    pub task: String,
    /// Start of the time range.
    pub from: String,
    /// End date of the time range, for picked dates.
    pub to: String,
}

impl RunsFilters {
    /// Default time range: the last 24 hours, or all time when the page shows one job's runs.
    pub const fn default_from(scoped: bool) -> &'static str {
        if scoped {
            "any"
        } else {
            "24h"
        }
    }

    /// Filters from the URL query values, with the default time range when `from` is missing.
    pub fn from_query(
        status: Option<String>,
        task: Option<String>,
        from: Option<String>,
        to: Option<String>,
        scoped: bool,
    ) -> Self {
        Self {
            status: status.unwrap_or_default(),
            task: task.unwrap_or_default(),
            from: from
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| Self::default_from(scoped).to_string()),
            to: to.unwrap_or_default(),
        }
    }

    /// Query string for these filters (without `?`), keeping `job` first when set.
    pub fn to_query(&self, job: Option<&str>) -> String {
        let pairs = [
            ("job", job.unwrap_or_default()),
            ("status", self.status.as_str()),
            ("task", self.task.as_str()),
            ("from", self.from.as_str()),
            ("to", self.to.as_str()),
        ];
        pairs
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| format!("{k}={}", boson_backend::encode_ops_path_segment(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// The time Select's value: a preset, or [`PICK_DATES`] when `from` is a date.
    fn time_key(&self) -> String {
        if self.from.contains('-') {
            PICK_DATES.to_string()
        } else {
            self.from.clone()
        }
    }

    /// The range in words for the footer and the no-match bar, e.g. "in the last 24 hours".
    pub fn time_phrase(&self) -> String {
        match self.from.as_str() {
            "1h" => "in the last hour".to_string(),
            "24h" => "in the last 24 hours".to_string(),
            "7d" => "in the last 7 days".to_string(),
            "30d" => "in the last 30 days".to_string(),
            "any" | "" => String::new(),
            from if self.to.is_empty() || self.to == from => format!("on {from}"),
            from => format!("from {from} to {}", self.to),
        }
    }

    /// Filter rules the table sends with every page request.
    fn to_table_filter(&self) -> DataTableFilter {
        let rule = |field: &str, value: &str| FilterRule {
            field: field.to_string(),
            operator: FilterOperator::Is,
            value: DataValue::Text(value.to_string()),
        };
        let mut items = Vec::new();
        if !self.status.is_empty() {
            items.push(rule("status", &self.status));
        }
        if !self.task.is_empty() {
            items.push(rule("task_name", &self.task));
        }
        if !self.from.is_empty() && self.from != "any" {
            items.push(rule(boson_backend::FROM_FIELD, &self.from));
        }
        if !self.to.is_empty() {
            items.push(rule(boson_backend::TO_FIELD, &self.to));
        }
        DataTableFilter {
            items,
            logic: FilterLogic::And,
        }
    }
}

fn date_value(date: &str) -> Option<OrbitalDateTime> {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()
        .and_then(|d| OrbitalDateTime::from_naive_date(d, DatetimeTimezone::Utc))
}

fn date_key(value: Option<OrbitalDateTime>) -> String {
    value
        .and_then(|v| v.wall_date())
        .map_or_else(String::new, |d| d.format("%Y-%m-%d").to_string())
}

fn today_key() -> String {
    chrono::Utc::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

/// The toolbar's control signals.
#[derive(Clone, Copy)]
struct RunsControls {
    status: RwSignal<String>,
    task: RwSignal<String>,
    time: RwSignal<String>,
    from_date: RwSignal<Option<OrbitalDateTime>>,
    to_date: RwSignal<Option<OrbitalDateTime>>,
}

/// Toolbar controls that follow the URL's filters and report changes to `on_filters_change`.
fn runs_controls(
    filters: Signal<RunsFilters>,
    on_filters_change: Callback<RunsFilters>,
) -> RunsControls {
    // The URL is the source of truth; the controls follow it.
    let initial = filters.get_untracked();
    let status = RwSignal::new(initial.status.clone());
    let task = RwSignal::new(initial.task.clone());
    let time = RwSignal::new(initial.time_key());
    let from_date = RwSignal::new(date_value(&initial.from));
    let to_date = RwSignal::new(date_value(&initial.to));

    Effect::new(move |_| {
        let f = filters.get();
        if status.get_untracked() != f.status {
            status.set(f.status.clone());
        }
        if task.get_untracked() != f.task {
            task.set(f.task.clone());
        }
        let key = f.time_key();
        if time.get_untracked() != key {
            time.set(key);
        }
        if date_key(from_date.get_untracked()) != f.from && f.from.contains('-') {
            from_date.set(date_value(&f.from));
        }
        if date_key(to_date.get_untracked()) != f.to {
            to_date.set(date_value(&f.to));
        }
    });

    Effect::new(move |_| {
        let time_key = time.get();
        let (from, to) = if time_key == PICK_DATES {
            let from = date_key(from_date.get());
            let from = if from.is_empty() { today_key() } else { from };
            let to = date_key(to_date.get());
            (from.clone(), if to.is_empty() { from } else { to })
        } else {
            (time_key, String::new())
        };
        let next = RunsFilters {
            status: status.get(),
            task: task.get(),
            from,
            to,
        };
        if next != filters.get_untracked() {
            on_filters_change.run(next);
        }
    });

    RunsControls {
        status,
        task,
        time,
        from_date,
        to_date,
    }
}

/// Runs list DataTable with status, task and time filters, id search, and a total.
#[component]
pub fn RunsDataTable(
    /// Scope to filter by.
    scope: RunsTableScope,
    /// Current toolbar filters (from the URL).
    #[prop(into)]
    filters: Signal<RunsFilters>,
    /// Called with new filters when a toolbar control changes; the page writes the URL.
    on_filters_change: Callback<RunsFilters>,
    /// Registered task names for the task Select.
    #[prop(into)]
    task_names: Signal<Vec<String>>,
    /// Page size.
    #[prop(default = RUNS_TABLE_PAGE_SIZE)]
    page_size: u32,
    /// Whether fill height is enabled.
    #[prop(default = true)]
    fill_height: bool,
    /// Optional empty description.
    #[prop(optional)]
    empty_description: Option<&'static str>,
) -> impl IntoView {
    let navigate = use_navigate();

    let on_row_click = Callback::new(move |(id,): (String,)| {
        navigate(
            &boson_backend::boson_run_path(&id),
            NavigateOptions::default(),
        );
    });

    let data_source = DataTableSource::Server {
        fetcher: build_runs_fetcher(scope),
        page_size,
    };

    let empty_desc = empty_description.unwrap_or("Run history will appear here when jobs execute.");

    let RunsControls {
        status,
        task,
        time,
        from_date,
        to_date,
    } = runs_controls(filters, on_filters_change);

    let table_filter = Signal::derive(move || Some(filters.get().to_table_filter()));

    let locale = DataTableLocale {
        quick_search_placeholder: "Search by run or job id".to_string(),
        ..DataTableLocale::default()
    };

    view! {
        <BosonDataTableShell data_testid="boson-runs-data-table">
            <DataTable
                data_source=data_source
                paging=PagingMode::Paged
                flex=fill_height
                columns=runs_table_columns()
                sortable=false
                features=DataTableFeatures::empty()
                filter=table_filter
                locale=locale
                header_chrome=DataTableHeaderChromeConfig {
                    column_menu: false,
                    column_filter_button: false,
                    column_hide: false,
                    show_table_grid: true,
                }
                events=DataTableEvents {
                    on_row_click: Some(on_row_click),
                    ..Default::default()
                }
            >
                <DataTableToolbarSlot slot>
                    <RunsToolbar
                        status=status
                        task=task
                        time=time
                        from_date=from_date
                        to_date=to_date
                        task_names=task_names
                    />
                </DataTableToolbarSlot>
                <DataTableFooterSlot slot>
                    <RunsFooter filters=filters />
                </DataTableFooterSlot>
                <DataTableLoadingView slot>
                    <BosonDataTableSkeletonRows columns=RUNS_TABLE_COLUMNS />
                </DataTableLoadingView>
                <DataTableEmptyView slot>
                    <EmptyState
                        message="No runs"
                        description=empty_desc
                    />
                </DataTableEmptyView>
                <DataTableNoResultsView slot>
                    <RunsNoMatch filters=filters on_filters_change=on_filters_change />
                </DataTableNoResultsView>
            </DataTable>
        </BosonDataTableShell>
    }
}

/// Search, then the status, task and time Selects (and date pickers for picked dates).
#[component]
fn RunsToolbar(
    status: RwSignal<String>,
    task: RwSignal<String>,
    time: RwSignal<String>,
    from_date: RwSignal<Option<OrbitalDateTime>>,
    to_date: RwSignal<Option<OrbitalDateTime>>,
    task_names: Signal<Vec<String>>,
) -> impl IntoView {
    let state = use_data_table_table_state();
    view! {
        <Flex wrap=FlexWrap::Wrap align=FlexAlign::Center gap=SpacingSize::Size80.flex_gap()>
            <DataTableQuickSearch state=state />
            <div data-testid="runs-status-filter">
                <Select bind=status>
                    <option value="">"Any status"</option>
                    <option value="running">"Running"</option>
                    <option value="success">"Succeeded"</option>
                    <option value="failed">"Failed"</option>
                    <option value="timeout">"Timed out"</option>
                    <option value="canceled">"Canceled"</option>
                </Select>
            </div>
            <div data-testid="runs-task-filter">
                <Select bind=task>
                    <option value="">"Any task"</option>
                    {move || {
                        task_names
                            .get()
                            .into_iter()
                            .map(|name| {
                                let label = name.clone();
                                view! { <option value=name>{label}</option> }
                            })
                            .collect_view()
                    }}
                </Select>
            </div>
            <div data-testid="runs-time-filter">
                <Select bind=time>
                    <option value="1h">"Last hour"</option>
                    <option value="24h">"Last 24 hours"</option>
                    <option value="7d">"Last 7 days"</option>
                    <option value="30d">"Last 30 days"</option>
                    <option value="any">"Any time"</option>
                    <option value=PICK_DATES>"Pick dates…"</option>
                </Select>
            </div>
            <Show when=move || time.get() == PICK_DATES>
                <div data-testid="runs-from-date">
                    <DatePicker bind=from_date />
                </div>
                <div data-testid="runs-to-date">
                    <DatePicker bind=to_date />
                </div>
            </Show>
        </Flex>
    }
}

/// The total ("4,812 runs in the last 24 hours") beside the paginator.
#[component]
fn RunsFooter(filters: Signal<RunsFilters>) -> impl IntoView {
    let state = use_data_table_table_state();
    let label = move || {
        let phrase = filters.get().time_phrase();
        state.server_total.get().map_or_else(String::new, |n| {
            let count = count_noun(n, "run", "runs");
            if phrase.is_empty() {
                count
            } else {
                format!("{count} {phrase}")
            }
        })
    };
    view! {
        <div class="orbital-data-table__footer" data-testid="data-table-footer">
            <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center wrap=FlexWrap::Wrap>
                <span data-testid="runs-total">
                    <Caption1>{label}</Caption1>
                </span>
                <DataTablePaginationBar state=state />
            </Flex>
        </div>
    }
}

/// Nothing matched: repeat what was searched, and offer Clear filters.
#[component]
fn RunsNoMatch(
    filters: Signal<RunsFilters>,
    on_filters_change: Callback<RunsFilters>,
) -> impl IntoView {
    let state = use_context::<DataTableTableState>();
    let message = move || {
        let searched = state.map_or_else(String::new, |s| s.quick_search.get().trim().to_string());
        let phrase = filters.get().time_phrase();
        match (searched.is_empty(), phrase.is_empty()) {
            (true, true) => "No runs match these filters.".to_string(),
            (true, false) => format!("No runs match these filters {phrase}."),
            (false, true) => format!("No runs match “{searched}”."),
            (false, false) => format!("No runs match “{searched}” {phrase}."),
        }
    };
    view! {
        <div data-testid="runs-no-match">
            <MessageBar intent=MessageBarIntent::Info>
                <Flex align=FlexAlign::Center gap=SpacingSize::Size120.flex_gap() wrap=FlexWrap::Wrap>
                    <span>{message}</span>
                    <Button
                        appearance=ButtonAppearance::Secondary
                        on_click=Callback::new(move |_| {
                            if let Some(s) = state {
                                s.quick_search.set(String::new());
                            }
                            // Empty filters: the page drops them from the URL, which restores
                            // the default range.
                            on_filters_change.run(RunsFilters::default());
                        })
                    >
                        "Clear filters"
                    </Button>
                </Flex>
            </MessageBar>
        </div>
    }
}
