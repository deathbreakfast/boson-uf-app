use std::sync::Arc;

use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use leptos_router::NavigateOptions;
use orbital::components::{
    Body1, Breadcrumb, BreadcrumbButton, BreadcrumbDivider, BreadcrumbItem, Caption1, Card,
    ContentContainer, EmptyState, Skeleton, SkeletonItem, SkeletonItemSize, SpacingSize, StatCard,
    Tab, TabList, Title3,
};
use orbital::primitives::{
    Button, ButtonAppearance, DataRecord, DataTable, DataTableColumnDef, DataTableEmptyView,
    DataTableFeatures, DataTableRowModel, DataTableSource, Flex, FlexAlign, FlexJustify, FlexWrap,
    Link, MessageBar, MessageBarIntent, PagingMode,
};

use uf_product::access::{AccessCard, AccessDenied};

use super::purge_dialog::PurgeDialog;
use super::QueueStateBadge;
use crate::components::{
    no_toolbar, row_model, text_cell, BosonCardContent, BosonHelpCardHeader, JobStatusBadge,
};
use crate::permissions::{denied_resource, queue_ref};
use crate::server::{
    get_queue, pause_queue, resume_queue, JobStatusDto, JobSummary, QueueDetail, QueueStateDto,
    QueueSummary,
};

/// Overview tab value (also the tab when `?tab=` is missing or unknown).
const TAB_OVERVIEW: &str = "overview";
/// Access tab value (`?tab=access`).
const TAB_ACCESS: &str = "access";

/// The tab named by a `?tab=` value.
fn tab_from_query(raw: Option<&str>) -> &'static str {
    match raw {
        Some(TAB_ACCESS) => TAB_ACCESS,
        _ => TAB_OVERVIEW,
    }
}

/// Queue page href for a tab (Overview has no query).
fn tab_href(queue: &str, tab: &str) -> String {
    let base = boson_backend::boson_queue_path(queue);
    if tab == TAB_OVERVIEW {
        base
    } else {
        format!("{base}?tab={tab}")
    }
}

/// The Overview and Access tabs.
#[component]
fn QueueTabs(
    /// Selected tab value.
    selected_tab: RwSignal<String>,
) -> impl IntoView {
    view! {
        <div data-testid="queue-detail-tabs">
            <TabList selected_value=selected_tab>
                <Tab value=TAB_OVERVIEW>"Overview"</Tab>
                <Tab value=TAB_ACCESS>"Access"</Tab>
            </TabList>
        </div>
    }
}

/// The shared Access card for one queue.
#[component]
fn QueueAccessTab(
    /// Queue name.
    name: String,
) -> impl IntoView {
    let resource = queue_ref(&name).label(name);
    view! {
        <div data-testid="queue-detail-access">
            <AccessCard resource=resource />
        </div>
    }
}

const fn status_key(status: JobStatusDto) -> &'static str {
    match status {
        JobStatusDto::Queued => "queued",
        JobStatusDto::Running => "running",
        JobStatusDto::Success => "success",
        JobStatusDto::Failed => "failed",
        JobStatusDto::Canceled => "canceled",
    }
}

fn status_from_key(key: &str) -> JobStatusDto {
    match key {
        "running" => JobStatusDto::Running,
        "success" => JobStatusDto::Success,
        "failed" => JobStatusDto::Failed,
        "canceled" => JobStatusDto::Canceled,
        _ => JobStatusDto::Queued,
    }
}

fn job_columns() -> Vec<DataTableColumnDef> {
    let id_view = Arc::new(|record: DataRecord| {
        let id = text_cell(&record, "job_id");
        let href = boson_backend::boson_runs_job_filter_path(&id);
        let testid = format!("queue-job-{id}");
        view! { <Link href=href attr:data-testid=testid>{id}</Link> }.into_any()
    });
    let status_view = Arc::new(|record: DataRecord| {
        let status = status_from_key(&text_cell(&record, "status"));
        view! { <JobStatusBadge status=status /> }.into_any()
    });
    vec![
        DataTableColumnDef::new("job_id", "Job")
            .with_sortable(false)
            .with_cell_view(id_view),
        DataTableColumnDef::new("task", "Task").with_sortable(false),
        DataTableColumnDef::new("status", "Status")
            .with_sortable(false)
            .with_cell_view(status_view),
        DataTableColumnDef::new("waited", "Enqueued").with_sortable(false),
    ]
}

fn job_rows(jobs: &[JobSummary]) -> Vec<DataTableRowModel> {
    let now = chrono::Utc::now();
    jobs.iter()
        .map(|j| {
            let waited = chrono::DateTime::parse_from_rfc3339(&j.created_at).map_or_else(
                |_| j.created_at.clone(),
                |t| format!("{} ago", boson_backend::wait_label(t.into(), now)),
            );
            row_model(
                &j.job_id,
                &[
                    ("job_id", j.job_id.clone()),
                    ("task", j.task_name.clone()),
                    ("status", status_key(j.status).to_string()),
                    ("waited", waited),
                ],
            )
        })
        .collect()
}

#[component]
fn QueueStats(
    /// Numbers to show.
    summary: QueueSummary,
) -> impl IntoView {
    let now = chrono::Utc::now();
    let oldest = boson_backend::oldest_waiting_label(summary.oldest_waiting_at, now);
    view! {
        <Flex gap=SpacingSize::Size160.flex_gap() wrap=FlexWrap::Wrap>
            <div data-testid="queue-stat-waiting">
                <StatCard label="Waiting" value={Signal::<String>::from(summary.waiting.to_string())} />
            </div>
            <div data-testid="queue-stat-running">
                <StatCard label="Running" value={Signal::<String>::from(summary.running.to_string())} />
            </div>
            <div data-testid="queue-stat-oldest">
                <StatCard label="Oldest waiting" value={Signal::<String>::from(oldest)} />
            </div>
            <div data-testid="queue-stat-done">
                <StatCard label="Done last hour" value={Signal::<String>::from(summary.done_last_hour.to_string())} />
            </div>
            <div data-testid="queue-stat-added">
                <StatCard label="Added last hour" value={Signal::<String>::from(summary.added_last_hour.to_string())} />
            </div>
        </Flex>
    }
}

fn paused_sentence(summary: &QueueSummary) -> String {
    let who = summary
        .paused_by
        .as_deref()
        .filter(|w| !w.is_empty())
        .map_or_else(|| "Paused".to_string(), |w| format!("Paused by {w}"));
    let when = summary
        .paused_at
        .map_or_else(String::new, |t| format!(" at {} UTC", t.format("%H:%M")));
    format!(
        "{who}{when}. Waiting jobs stay put; running jobs finish. New jobs still join the queue."
    )
}

/// Header, tabs and the selected tab for a loaded queue.
///
/// Pause and Resume show only with Edit on the queue, Purge only with Delete.
#[component]
fn QueueOverview(
    /// Loaded detail.
    detail: QueueDetail,
    /// Selected tab value.
    selected_tab: RwSignal<String>,
    /// True while pause, resume or purge is in flight.
    busy: ReadSignal<bool>,
    /// Pause when running, resume when paused.
    on_toggle_pause: Callback<bool>,
    /// Opens the purge dialog.
    on_purge: Callback<()>,
) -> impl IntoView {
    let summary = detail.summary;
    let can_edit = detail.can_edit;
    let can_delete = detail.can_delete;
    let paused = summary.state == QueueStateDto::Paused;
    let name = summary.name.clone();
    let jobs = RwSignal::new(job_rows(&detail.jobs));
    let paused_bar = paused.then(|| paused_sentence(&summary));
    let title = name.clone();
    let owner = format!("Owner: {}", summary.owner);
    let all_jobs_href = boson_backend::boson_queue_jobs_path(&name);
    let all_jobs_label = format!("All {name} jobs");
    let (pause_label, pause_appearance) = if paused {
        ("Resume", ButtonAppearance::Primary)
    } else {
        ("Pause", ButtonAppearance::Secondary)
    };

    view! {
        <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
            <Flex
                justify=FlexJustify::SpaceBetween
                align=FlexAlign::Center
                wrap=FlexWrap::Wrap
                gap=SpacingSize::Size120.flex_gap()
            >
                <Flex vertical=true gap=SpacingSize::Size40.flex_gap()>
                    <Flex align=FlexAlign::Center gap=SpacingSize::Size120.flex_gap()>
                        <Title3>{title}</Title3>
                        <span data-testid="queue-detail-state">
                            <QueueStateBadge state=summary.state />
                        </span>
                    </Flex>
                    <Caption1>{owner}</Caption1>
                </Flex>
                <Flex gap=SpacingSize::Size80.flex_gap()>
                    {can_edit.then(|| view! {
                        <div data-testid="queue-pause-toggle">
                            <Button
                                appearance=pause_appearance
                                disabled=Signal::derive(move || busy.get())
                                on:click=move |_| on_toggle_pause.run(!paused)
                            >
                                {pause_label}
                            </Button>
                        </div>
                    })}
                    {can_delete.then(|| view! {
                        <div data-testid="queue-purge-open">
                            <Button
                                appearance=ButtonAppearance::Danger
                                disabled=Signal::derive(move || busy.get())
                                on:click=move |_| on_purge.run(())
                            >
                                "Purge waiting jobs"
                            </Button>
                        </div>
                    })}
                </Flex>
            </Flex>

            <QueueTabs selected_tab=selected_tab />

            {move || if selected_tab.get() == TAB_ACCESS {
                view! { <QueueAccessTab name=name.clone() /> }.into_any()
            } else {
                view! {
                    <QueueOverviewBody
                        summary=summary.clone()
                        paused_bar=paused_bar.clone()
                        jobs=jobs
                        all_jobs_href=all_jobs_href.clone()
                        all_jobs_label=all_jobs_label.clone()
                    />
                }.into_any()
            }}
        </Flex>
    }
}

/// Overview tab: the paused note, the numbers and what's waiting and running.
#[component]
fn QueueOverviewBody(
    /// Numbers to show.
    summary: QueueSummary,
    /// The paused sentence, when the queue is paused.
    paused_bar: Option<String>,
    /// Waiting and running job rows.
    jobs: RwSignal<Vec<DataTableRowModel>>,
    /// Link to every job of this queue.
    all_jobs_href: String,
    /// Text of that link.
    all_jobs_label: String,
) -> impl IntoView {
    view! {
        <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
            {paused_bar.map(|text| view! {
                <div data-testid="queue-paused-bar">
                    <MessageBar intent=MessageBarIntent::Warning>{text}</MessageBar>
                </div>
            })}

            <QueueStats summary=summary />

            <Card>
                <BosonHelpCardHeader title="Waiting and running" />
                <BosonCardContent>
                    <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                        <div data-testid="queue-jobs-table">
                            <DataTable
                                data_source=DataTableSource::Client(jobs)
                                paging=PagingMode::None
                                features=DataTableFeatures::empty()
                                toolbar_config=no_toolbar()
                                sortable=false
                                columns=job_columns()
                            >
                                <DataTableEmptyView slot>
                                    <EmptyState
                                        message="Nothing waiting or running"
                                        description="Jobs show up here while they wait for a worker or run."
                                    />
                                </DataTableEmptyView>
                            </DataTable>
                        </div>
                        <Flex justify=FlexJustify::End>
                            <Link
                                href=all_jobs_href
                                attr:data-testid="queue-all-jobs"
                            >
                                {all_jobs_label}
                            </Link>
                        </Flex>
                    </Flex>
                </BosonCardContent>
            </Card>
        </Flex>
    }
}

#[component]
fn QueueOverviewSkeleton() -> impl IntoView {
    view! {
        <Skeleton>
            <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                {(0..4)
                    .map(|_| view! { <SkeletonItem size=Signal::from(SkeletonItemSize::S24) /> })
                    .collect_view()}
            </Flex>
        </Skeleton>
    }
}

/// The queue's name and tabs with the denied state in place of the numbers and jobs, for a
/// reader without View. The Access tab still shows who has access, read only.
#[component]
fn QueueDenied(
    /// Queue name.
    name: String,
    /// The missing action the server named.
    action: String,
    /// Selected tab value.
    selected_tab: RwSignal<String>,
) -> impl IntoView {
    let resource = queue_ref(&name).label(name.clone());
    let title = name.clone();
    view! {
        <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
            <Title3>{title}</Title3>
            <QueueTabs selected_tab=selected_tab />
            {move || if selected_tab.get() == TAB_ACCESS {
                view! { <QueueAccessTab name=name.clone() /> }.into_any()
            } else {
                view! {
                    <div data-testid="queue-detail-denied">
                        <AccessDenied
                            resource=resource.clone()
                            action=action.clone()
                            purpose="see what's waiting and running"
                        />
                    </div>
                }.into_any()
            }}
        </Flex>
    }
}

/// The selected tab, kept in step with `?tab=` both ways: back and forward move the tab, and a
/// tab click writes the URL without scrolling or adding history.
fn use_queue_tab(name: Memo<String>) -> RwSignal<String> {
    let query = use_query_map();
    let navigate = use_navigate();
    let tab_in_url = Memo::new(move |_| tab_from_query(query.read().get("tab").as_deref()));
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
                &tab_href(&name.get_untracked(), &tab),
                NavigateOptions {
                    replace: true,
                    scroll: false,
                    ..Default::default()
                },
            );
        }
    });
    selected_tab
}

/// `/boson/queues/:name`: one queue's numbers, what's waiting and running, and the actions,
/// with an Access tab. Switching tabs changes `?tab=` without reloading the page.
#[component]
pub fn BosonQueueDetailPage() -> impl IntoView {
    let params = use_params_map();
    let name = Memo::new(move |_| params.read().get("name").unwrap_or_default());
    let selected_tab = use_queue_tab(name);
    let refresh = RwSignal::new(0u32);
    let detail_res = Resource::new(
        move || (name.get(), refresh.get()),
        |(name, _)| async move { get_queue(name).await },
    );
    let busy = RwSignal::new(false);
    let action_error = RwSignal::new(None::<String>);
    let purge_open = RwSignal::new(false);

    let on_toggle_pause = Callback::new(move |pause: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        action_error.set(None);
        let queue = name.get_untracked();
        spawn_local_scoped(async move {
            let result = if pause {
                pause_queue(queue).await
            } else {
                resume_queue(queue).await
            };
            if let Err(e) = result {
                action_error.set(Some(e.to_string()));
            }
            busy.set(false);
            refresh.update(|n| *n += 1);
        });
    });
    let on_purged = Callback::new(move |()| refresh.update(|n| *n += 1));

    view! {
        <ContentContainer data_testid="boson-queue-detail">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <Breadcrumb>
                    <BreadcrumbItem>
                        <Link href=boson_backend::BOSON_QUEUES_PATH>
                            <BreadcrumbButton>"Queues"</BreadcrumbButton>
                        </Link>
                    </BreadcrumbItem>
                    <BreadcrumbDivider />
                    <BreadcrumbItem>
                        <BreadcrumbButton current=true>{move || name.get()}</BreadcrumbButton>
                    </BreadcrumbItem>
                </Breadcrumb>

                {move || action_error.get().map(|e| view! {
                    <MessageBar intent=MessageBarIntent::Error>{e}</MessageBar>
                })}

                <Transition fallback=move || view! { <QueueOverviewSkeleton /> }>
                    {move || detail_res.get().map(|r| match r {
                        Ok(detail) => {
                            let summary = detail.summary.clone();
                            view! {
                                <QueueOverview
                                    detail=detail
                                    selected_tab=selected_tab
                                    busy=busy.read_only()
                                    on_toggle_pause=on_toggle_pause
                                    on_purge=Callback::new(move |()| purge_open.set(true))
                                />
                                <PurgeDialog open=purge_open summary=summary on_purged=on_purged />
                            }.into_any()
                        }
                        Err(e) => match denied_resource(&e.to_string()) {
                            Some((_, action)) => view! {
                                <QueueDenied name=name.get_untracked() action=action selected_tab=selected_tab />
                            }.into_any(),
                            None => view! {
                                <MessageBar intent=MessageBarIntent::Error>
                                    <Body1>{format!("Couldn't load this queue. {e}")}</Body1>
                                </MessageBar>
                            }.into_any(),
                        },
                    })}
                </Transition>
            </Flex>
        </ContentContainer>
    }
}
