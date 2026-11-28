use std::sync::Arc;

use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use leptos_router::hooks::use_params_map;
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

use super::purge_dialog::PurgeDialog;
use super::QueueStateBadge;
use crate::components::{
    no_toolbar, row_model, text_cell, BosonCardContent, BosonHelpCardHeader, JobStatusBadge,
};
use crate::server::{
    get_queue, pause_queue, resume_queue, JobStatusDto, JobSummary, QueueDetail, QueueStateDto,
    QueueSummary,
};

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

#[component]
fn QueueOverview(
    /// Loaded detail.
    detail: QueueDetail,
    /// True while pause, resume or purge is in flight.
    busy: ReadSignal<bool>,
    /// Pause when running, resume when paused.
    on_toggle_pause: Callback<bool>,
    /// Opens the purge dialog.
    on_purge: Callback<()>,
) -> impl IntoView {
    let summary = detail.summary;
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
                    <div data-testid="queue-pause-toggle">
                        <Button
                            appearance=pause_appearance
                            disabled=Signal::derive(move || busy.get())
                            on:click=move |_| on_toggle_pause.run(!paused)
                        >
                            {pause_label}
                        </Button>
                    </div>
                    <div data-testid="queue-purge-open">
                        <Button
                            appearance=ButtonAppearance::Danger
                            disabled=Signal::derive(move || busy.get())
                            on:click=move |_| on_purge.run(())
                        >
                            "Purge waiting jobs"
                        </Button>
                    </div>
                </Flex>
            </Flex>

            <TabList selected_value=RwSignal::new("overview".to_string())>
                <Tab value="overview">"Overview"</Tab>
            </TabList>

            {paused_bar.map(|text| view! {
                <div data-testid="queue-paused-bar">
                    <MessageBar intent=MessageBarIntent::Warning>{text}</MessageBar>
                </div>
            })}

            <QueueStats summary=summary.clone() />

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

/// `/boson/queues/:name`: one queue's numbers, what's waiting and running, and the actions.
#[component]
pub fn BosonQueueDetailPage() -> impl IntoView {
    let params = use_params_map();
    let name = Memo::new(move |_| params.read().get("name").unwrap_or_default());
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
                                    busy=busy.read_only()
                                    on_toggle_pause=on_toggle_pause
                                    on_purge=Callback::new(move |()| purge_open.set(true))
                                />
                                <PurgeDialog open=purge_open summary=summary on_purged=on_purged />
                            }.into_any()
                        }
                        Err(e) => view! {
                            <MessageBar intent=MessageBarIntent::Error>
                                <Body1>{format!("Couldn't load this queue. {e}")}</Body1>
                            </MessageBar>
                        }.into_any(),
                    })}
                </Transition>
            </Flex>
        </ContentContainer>
    }
}
