mod skeleton;

use leptos::prelude::*;
use leptos_router::components::Redirect;
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use leptos_router::NavigateOptions;
use orbital::components::{
    AutoGrid, Caption1, Card, ContentContainer, EmptyState, EmptyStateCallToAction, SpacingSize,
    StatCard, Subtitle2, Text, TextTag, Title3,
};
use orbital::primitives::{
    Body1, Breadcrumb, BreadcrumbButton, BreadcrumbDivider, BreadcrumbItem, CardHeader,
    CardHeaderAction, Flex, FlexAlign, Link, MessageBar, MessageBarIntent, Tab, TabList, Table,
    TableBody, TableCell, TableCellLayout, TableHeader, TableHeaderCell, TableRow,
};

use crate::components::{BosonBackLink, BosonCardContent, RunStatusBadge};
use crate::pages::task_config::BosonTaskConfigPage;
use crate::server::{get_task_detail, TaskArgumentRow, TaskDetail};

use skeleton::TaskDetailSkeleton;

/// Overview tab value (also the tab when `?tab=` is missing or unknown).
const TAB_OVERVIEW: &str = "overview";
/// Settings tab value (`?tab=settings`; `/config` redirects here).
const TAB_SETTINGS: &str = "settings";
/// Access tab value.
const TAB_ACCESS: &str = "access";

/// Owner text when the host supplies no owner for the task type.
const NO_OWNER: &str = "Not set yet";

/// The tab named by a `?tab=` value.
fn tab_from_query(raw: Option<&str>) -> &'static str {
    match raw {
        Some(TAB_SETTINGS) => TAB_SETTINGS,
        Some(TAB_ACCESS) => TAB_ACCESS,
        _ => TAB_OVERVIEW,
    }
}

/// Task type page href for a tab (Overview has no query).
fn tab_href(task_name: &str, tab: &str) -> String {
    let base = boson_backend::boson_task_path(task_name);
    if tab == TAB_OVERVIEW {
        base
    } else {
        format!("{base}?tab={tab}")
    }
}

/// Arguments from the signature as a table.
#[component]
fn ArgumentsCard(
    /// Rows from the signature JSON.
    arguments: Vec<TaskArgumentRow>,
) -> impl IntoView {
    view! {
        <Card>
            <CardHeader>
                <Subtitle2>"Arguments"</Subtitle2>
            </CardHeader>
            <BosonCardContent>
                <div data-testid="task-detail-arguments">
                    {if arguments.is_empty() {
                        view! { <Body1>"This task type doesn't describe its arguments."</Body1> }.into_any()
                    } else {
                        view! {
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHeaderCell>"Name"</TableHeaderCell>
                                        <TableHeaderCell>"Type"</TableHeaderCell>
                                        <TableHeaderCell>"Description"</TableHeaderCell>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {arguments.into_iter().map(|a| view! {
                                        <TableRow>
                                            <TableCell><TableCellLayout><Text tag=TextTag::Code>{a.name}</Text></TableCellLayout></TableCell>
                                            <TableCell><TableCellLayout>{a.type_label}</TableCellLayout></TableCell>
                                            <TableCell><TableCellLayout>{a.description.unwrap_or_default()}</TableCellLayout></TableCell>
                                        </TableRow>
                                    }).collect_view()}
                                </TableBody>
                            </Table>
                        }.into_any()
                    }}
                </div>
            </BosonCardContent>
        </Card>
    }
}

/// The task's latest runs with a link to all of them.
#[component]
fn RecentRunsCard(
    /// Loaded task detail.
    detail: TaskDetail,
) -> impl IntoView {
    let all_href = boson_backend::boson_task_runs_path(&detail.summary.name);
    let action = CardHeaderAction {
        children: Box::new(move || {
            view! {
                <span data-testid="task-detail-all-runs">
                    <Link href=all_href>"All runs of this task"</Link>
                </span>
            }
            .into_any()
        }),
    };
    let runs = detail.recent_runs;

    view! {
        <Card>
            <CardHeader card_header_action=action>
                <Subtitle2>"Recent runs"</Subtitle2>
            </CardHeader>
            <BosonCardContent>
                <div data-testid="task-detail-recent-runs">
                    {if runs.is_empty() {
                        view! { <Body1>"No runs yet."</Body1> }.into_any()
                    } else {
                        view! {
                            <Table>
                                <TableHeader>
                                    <TableRow>
                                        <TableHeaderCell>"Run"</TableHeaderCell>
                                        <TableHeaderCell>"Status"</TableHeaderCell>
                                        <TableHeaderCell>"Attempt"</TableHeaderCell>
                                        <TableHeaderCell>"Started"</TableHeaderCell>
                                        <TableHeaderCell>"Duration"</TableHeaderCell>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {runs.into_iter().map(|r| {
                                        let href = boson_backend::boson_run_path(&r.run_id);
                                        let short = boson_backend::short_id(&r.run_id);
                                        let duration = r.duration_ms.map_or_else(
                                            || "-".to_string(),
                                            boson_backend::format_duration_ms,
                                        );
                                        view! {
                                            <TableRow>
                                                <TableCell><TableCellLayout><Link href=href>{short}</Link></TableCellLayout></TableCell>
                                                <TableCell><TableCellLayout><RunStatusBadge status=r.status /></TableCellLayout></TableCell>
                                                <TableCell><TableCellLayout>{r.attempt}</TableCellLayout></TableCell>
                                                <TableCell><TableCellLayout>{r.started_at}</TableCellLayout></TableCell>
                                                <TableCell><TableCellLayout>{duration}</TableCellLayout></TableCell>
                                            </TableRow>
                                        }
                                    }).collect_view()}
                                </TableBody>
                            </Table>
                        }.into_any()
                    }}
                </div>
            </BosonCardContent>
        </Card>
    }
}

/// Overview tab: stats, the settings sentence, arguments and recent runs.
#[component]
fn OverviewTab(
    /// Loaded task detail.
    detail: TaskDetail,
) -> impl IntoView {
    let waiting = detail.waiting.to_string();
    let runs_24h = detail.runs_24h.to_string();
    let success = detail
        .success_rate_24h_pct
        .map_or_else(|| "No runs yet".to_string(), |p| format!("{p:.1}%"));
    let queue = detail.summary.effective_pool.clone();
    let sentence = detail.settings_sentence.clone();
    let arguments = detail.arguments.clone();

    view! {
        <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
            <div data-testid="task-detail-stats">
                <AutoGrid min="180px">
                    <StatCard label="Waiting" value=Signal::stored(waiting) />
                    <StatCard label="Runs (24h)" value=Signal::stored(runs_24h) />
                    <StatCard label="Success rate (24h)" value=Signal::stored(success) />
                    <StatCard label="Queue" value=Signal::stored(queue) />
                </AutoGrid>
            </div>
            <div data-testid="task-detail-settings-sentence">
                <Caption1>{sentence}</Caption1>
            </div>
            <ArgumentsCard arguments=arguments />
            <RecentRunsCard detail=detail />
        </Flex>
    }
}

/// Header, tabs and the selected tab for a loaded task type.
#[component]
fn TaskDetailBody(
    /// Loaded task detail.
    detail: TaskDetail,
    /// Selected tab value.
    selected_tab: RwSignal<String>,
) -> impl IntoView {
    let name = detail.summary.name.clone();
    let owner = detail
        .summary
        .owner
        .clone()
        .unwrap_or_else(|| NO_OWNER.to_string());
    let header_line = detail.description.as_ref().map_or_else(
        || format!("Owner: {owner}"),
        |d| format!("{d} Owner: {owner}"),
    );
    let waiting_href = boson_backend::boson_task_waiting_jobs_path(&name);
    let failed_href = boson_backend::boson_task_failed_runs_path(&name);
    let waiting_label = format!("{} waiting", detail.waiting);
    let can_edit = detail.can_edit;
    let default_priority = detail.summary.default_priority;
    let task_name = Signal::stored(name.clone());
    let detail = StoredValue::new(detail);

    view! {
        <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
            <Flex vertical=true gap=SpacingSize::Size40.flex_gap()>
                <Title3>{name}</Title3>
                <div data-testid="task-detail-owner">
                    <Caption1>{header_line}</Caption1>
                </div>
                <Flex align=FlexAlign::Center gap=SpacingSize::Size160.flex_gap()>
                    <span data-testid="task-detail-waiting">
                        <Link href=waiting_href>{waiting_label}</Link>
                    </span>
                    <span data-testid="task-detail-failed-runs">
                        <Link href=failed_href>"Failed runs"</Link>
                    </span>
                </Flex>
            </Flex>
            <div data-testid="task-detail-tabs">
                <TabList selected_value=selected_tab>
                    <Tab value=TAB_OVERVIEW>"Overview"</Tab>
                    {can_edit.then(|| view! { <Tab value=TAB_SETTINGS>"Settings"</Tab> })}
                    <Tab value=TAB_ACCESS>"Access"</Tab>
                </TabList>
            </div>
            {move || {
                let tab = selected_tab.get();
                if tab == TAB_SETTINGS && can_edit {
                    view! {
                        <orbital::routes::RequireAuthenticated requires_email_verification=true>
                            <BosonTaskConfigPage task_name=task_name default_priority=default_priority />
                        </orbital::routes::RequireAuthenticated>
                    }
                    .into_any()
                } else if tab == TAB_ACCESS {
                    view! {
                        <Card>
                            <BosonCardContent>
                                <div data-testid="task-detail-access">
                                    <Body1>"Who can see and change this task type will show here."</Body1>
                                </div>
                            </BosonCardContent>
                        </Card>
                    }
                    .into_any()
                } else {
                    view! { <OverviewTab detail=detail.get_value() /> }.into_any()
                }
            }}
        </Flex>
    }
}

/// Task type page: breadcrumb, owner, links to its waiting jobs and failed runs, and the
/// Overview, Settings and Access tabs. Switching tabs changes `?tab=` without reloading the page.
#[component]
pub fn BosonTaskDetailPage() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    let navigate = use_navigate();
    let task_name = Memo::new(move |_| params.get().get("task_name").unwrap_or_default());
    let tab_in_url = Memo::new(move |_| tab_from_query(query.read().get("tab").as_deref()));

    let task_res = Resource::new(
        move || task_name.get(),
        move |name| async move {
            if name.is_empty() {
                return Ok(None);
            }
            get_task_detail(name).await
        },
    );

    let selected_tab = RwSignal::new(tab_in_url.get_untracked().to_string());
    // Back/forward changes the URL: follow it.
    Effect::new(move |_| {
        let tab = tab_in_url.get();
        if selected_tab.get_untracked() != tab {
            selected_tab.set(tab.to_string());
        }
    });
    // A tab click: write `?tab=` without scrolling or adding history.
    Effect::new(move |_| {
        let tab = selected_tab.get();
        if tab != tab_in_url.get_untracked() {
            navigate(
                &tab_href(&task_name.get_untracked(), &tab),
                NavigateOptions {
                    replace: true,
                    scroll: false,
                    ..Default::default()
                },
            );
        }
    });

    view! {
        <ContentContainer data_testid="boson-task-detail">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <div data-testid="task-detail-breadcrumb">
                    <Breadcrumb>
                        <BreadcrumbItem>
                            <Link href=crate::paths::TASKS.to_string()>"Tasks"</Link>
                        </BreadcrumbItem>
                        <BreadcrumbDivider />
                        <BreadcrumbItem>
                            <BreadcrumbButton current=true>{move || task_name.get()}</BreadcrumbButton>
                        </BreadcrumbItem>
                    </Breadcrumb>
                </div>

                <Suspense fallback=move || view! { <TaskDetailSkeleton /> }>
                    {move || match task_res.get() {
                        Some(Ok(Some(d))) => view! {
                            <TaskDetailBody detail=d selected_tab=selected_tab />
                        }.into_any(),
                        Some(Ok(None)) => {
                            let name = task_name.get();
                            view! {
                                <Card>
                                    <BosonCardContent>
                                        <div data-testid="task-detail-not-found">
                                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                                <Body1>{format!("No task type called {name}.")}</Body1>
                                                <EmptyState
                                                    message="Task type not found"
                                                    description="Boson has no task type registered under this name on this host."
                                                    call_to_action=EmptyStateCallToAction {
                                                        children: Box::new(|| view! {
                                                            <BosonBackLink href=crate::paths::TASKS label="Back to Tasks" />
                                                        }.into_any()),
                                                    }
                                                />
                                            </Flex>
                                        </div>
                                    </BosonCardContent>
                                </Card>
                            }.into_any()
                        }
                        Some(Err(e)) => view! {
                            <MessageBar intent=MessageBarIntent::Error>{e.to_string()}</MessageBar>
                        }.into_any(),
                        None => view! { <TaskDetailSkeleton /> }.into_any(),
                    }}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}

/// `/boson/tasks/:task_name/config`: sends the old config URL to this page's Settings tab.
#[component]
pub fn BosonTaskConfigRedirect() -> impl IntoView {
    let params = use_params_map();
    let task_name = params.get_untracked().get("task_name").unwrap_or_default();
    view! { <Redirect path=boson_backend::boson_task_settings_path(&task_name) /> }
}
