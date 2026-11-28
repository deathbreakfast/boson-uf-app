mod attempts;
mod run_error_display;
mod run_info_grid;

pub use attempts::RunAttempts;
pub use run_error_display::RunErrorDisplay;
pub use run_info_grid::RunInfoGrid;

use leptos::prelude::*;
use leptos::server_fn::ServerFnError;
use leptos::task::spawn_local_scoped;
use leptos_router::hooks::use_params_map;
use orbital::components::{
    Caption1, Card, ContentContainer, Skeleton, SkeletonItem, SpacingSize, Title3,
};
use orbital::primitives::{
    Body1, Breadcrumb, BreadcrumbButton, BreadcrumbDivider, BreadcrumbItem, Button,
    ButtonAppearance, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle,
    Flex, FlexAlign, FlexJustify, Link, MessageBar, MessageBarIntent,
};
use orbital_motion::OrbitalPresence;

use crate::components::{
    boson_error_reveal_motion, server_fn_error_or_denied, BosonCardContent, RunStatusBadge,
};
use crate::live::{
    boson_job_run_subscription, boson_run_event_is_status, boson_run_event_matches_run,
    BosonJobRunLiveSource,
};
use crate::server::{
    cancel_job, drop_jobs, get_run_detail, retry_jobs, JobStatusDto, RunDetail, RunStatusDto,
};

/// Skeleton placeholder while run detail loads.
#[component]
fn RunDetailSkeleton() -> impl IntoView {
    view! {
        <Card>
            <BosonCardContent>
                <Skeleton>
                    <SkeletonItem width="100%".to_string() height="240px".to_string() />
                </Skeleton>
            </BosonCardContent>
        </Card>
    }
}

/// "Attempt 3 of 3 · the job has no attempts left", "Attempt 1 of 3 · running for 6 min".
fn attempt_line(detail: &RunDetail) -> String {
    let run = &detail.run;
    let mut line = format!("Attempt {} of {}", run.attempt, detail.max_attempts);
    let job_status = detail.job.as_ref().map(|j| j.status);
    match run.status {
        RunStatusDto::Running => {
            if let Some(ms) = detail.running_for_ms {
                line.push_str(" · running for ");
                line.push_str(&boson_backend::format_duration_ms(ms));
            }
        }
        RunStatusDto::Failed | RunStatusDto::Timeout => match job_status {
            Some(JobStatusDto::Failed) => line.push_str(" · the job has no attempts left"),
            Some(JobStatusDto::Queued | JobStatusDto::Running) => {
                line.push_str(" · the job will try again");
            }
            _ => {}
        },
        _ => {}
    }
    line
}

/// The reason a single-job Retry or Drop didn't go through.
fn outcome_error(failed: &[crate::server::JobActionFailure], verb: &str) -> String {
    failed.first().map_or_else(
        || format!("Couldn't {verb} the job."),
        |f| format!("Couldn't {verb} the job: {}.", f.error),
    )
}

/// Retry job and Drop (failed job, last attempt) or Cancel job (running job) for people who can edit.
#[component]
fn RunActions(
    /// Loaded run detail.
    detail: RunDetail,
    /// Refetch the page data after an action.
    on_changed: Callback<()>,
) -> impl IntoView {
    let Some(job) = detail.job.clone() else {
        return ().into_any();
    };
    if !detail.can_run {
        return ().into_any();
    }
    let job_id = StoredValue::new(job.job_id.clone());
    let action_error = RwSignal::new(None::<String>);
    let pending = RwSignal::new(false);

    let error_bar = move || {
        action_error.get().map(|e| {
            view! { <MessageBar intent=MessageBarIntent::Error>{e}</MessageBar> }
        })
    };

    if job.status == JobStatusDto::Running {
        let on_cancel = Callback::new(move |_| {
            pending.set(true);
            action_error.set(None);
            spawn_local_scoped(async move {
                let result = cancel_job(job_id.get_value()).await;
                pending.set(false);
                match result {
                    Ok(()) => on_changed.run(()),
                    Err(e) => action_error.set(Some(crate::errors::friendly_error(&e).sentence())),
                }
            });
        });
        return view! {
            <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                <div data-testid="run-detail-cancel">
                    <Button appearance=ButtonAppearance::Secondary disabled=pending on_click=on_cancel>
                        "Cancel job"
                    </Button>
                </div>
                {error_bar}
            </Flex>
        }
        .into_any();
    }

    let is_last_failed = job.status == JobStatusDto::Failed
        && matches!(
            detail.run.status,
            RunStatusDto::Failed | RunStatusDto::Timeout
        );
    if !is_last_failed {
        return ().into_any();
    }

    let confirm_open = RwSignal::new(false);
    let retried = RwSignal::new(None::<String>);
    let task_name = detail.run.task_name.clone();
    let max_attempts = detail.max_attempts;
    let last_error = detail
        .run
        .error_message
        .as_deref()
        .and_then(|e| e.lines().next())
        .map(str::to_string);

    let on_retry = Callback::new(move |_| {
        pending.set(true);
        action_error.set(None);
        spawn_local_scoped(async move {
            let result = retry_jobs(vec![job_id.get_value()]).await;
            pending.set(false);
            confirm_open.set(false);
            match result {
                Ok(outcome) => match outcome.retried.into_iter().next() {
                    Some(r) => retried.set(Some(r.job_id)),
                    None => action_error.set(Some(outcome_error(&outcome.failed, "retry"))),
                },
                Err(e) => action_error.set(Some(crate::errors::friendly_error(&e).sentence())),
            }
        });
    });

    let drop_open = RwSignal::new(false);
    let dropped = RwSignal::new(false);
    let on_drop = Callback::new(move |_| {
        pending.set(true);
        action_error.set(None);
        spawn_local_scoped(async move {
            let result = drop_jobs(vec![job_id.get_value()]).await;
            pending.set(false);
            drop_open.set(false);
            match result {
                Ok(outcome) if !outcome.dropped.is_empty() => dropped.set(true),
                Ok(outcome) => action_error.set(Some(outcome_error(&outcome.failed, "drop"))),
                Err(e) => action_error.set(Some(crate::errors::friendly_error(&e).sentence())),
            }
        });
    });
    let done = Signal::derive(move || retried.get().is_some() || dropped.get());

    view! {
        <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
            <Flex gap=SpacingSize::Size80.flex_gap()>
            <div data-testid="run-detail-retry">
                <Button
                    appearance=ButtonAppearance::Primary
                    disabled=done
                    on_click=Callback::new(move |_| confirm_open.set(true))
                >
                    "Retry job"
                </Button>
            </div>
            <div data-testid="run-detail-drop">
                <Button
                    appearance=ButtonAppearance::Secondary
                    disabled=done
                    on_click=Callback::new(move |_| drop_open.set(true))
                >
                    "Drop"
                </Button>
            </div>
            </Flex>
            {move || dropped.get().then(|| view! {
                <div data-testid="run-detail-dropped">
                    <MessageBar intent=MessageBarIntent::Success>
                        "Dropped. The job won't run again; its runs stay."
                    </MessageBar>
                </div>
            })}
            <Dialog open=drop_open>
                <DialogSurface>
                    <DialogBody>
                        <DialogTitle>"Drop this job?"</DialogTitle>
                        <DialogContent>
                            <Body1>"The job leaves the failed list and never runs again. Its runs and their errors stay."</Body1>
                        </DialogContent>
                        <DialogActions>
                            <Button
                                appearance=ButtonAppearance::Secondary
                                on_click=Callback::new(move |_| drop_open.set(false))
                            >
                                "Cancel"
                            </Button>
                            <div data-testid="run-detail-drop-confirm">
                                <Button appearance=ButtonAppearance::Primary disabled=pending on_click=on_drop>
                                    "Drop"
                                </Button>
                            </div>
                        </DialogActions>
                    </DialogBody>
                </DialogSurface>
            </Dialog>
            {error_bar}
            {move || retried.get().map(|new_id| {
                let href = boson_backend::boson_runs_job_filter_path(&new_id);
                let short = boson_backend::short_id(&new_id);
                view! {
                    <div data-testid="run-detail-retried">
                        <MessageBar intent=MessageBarIntent::Success>
                            "Retried. The new job "
                            <Link href=href>{short}</Link>
                            " is waiting to run."
                        </MessageBar>
                    </div>
                }
            })}
            <Dialog open=confirm_open>
                <DialogSurface>
                    <DialogBody>
                        <DialogTitle>"Retry this job?"</DialogTitle>
                        <DialogContent>
                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                <Body1>
                                    {format!(
                                        "Boson starts this {task_name} job again from attempt 1, with the same arguments and today's retry policy (up to {max_attempts} attempts). It runs as you."
                                    )}
                                </Body1>
                                {last_error.clone().map(|e| view! {
                                    <MessageBar intent=MessageBarIntent::Warning>
                                        {format!("The last attempt failed with “{e}”. If that cause hasn't been fixed, the new job will fail the same way.")}
                                    </MessageBar>
                                })}
                            </Flex>
                        </DialogContent>
                        <DialogActions>
                            <Button
                                appearance=ButtonAppearance::Secondary
                                on_click=Callback::new(move |_| confirm_open.set(false))
                            >
                                "Cancel"
                            </Button>
                            <div data-testid="run-detail-retry-confirm">
                                <Button appearance=ButtonAppearance::Primary disabled=pending on_click=on_retry>
                                    "Retry job"
                                </Button>
                            </div>
                        </DialogActions>
                    </DialogBody>
                </DialogSurface>
            </Dialog>
        </Flex>
    }
    .into_any()
}

/// Runs › task › Run …0412. The task crumb appears once the run loads.
#[component]
fn RunBreadcrumb(
    /// Run id from the path.
    run_id: Memo<String>,
    /// The page's run detail resource.
    run_res: Resource<Result<Option<RunDetail>, ServerFnError>>,
) -> impl IntoView {
    view! {
        <div data-testid="run-detail-breadcrumb">
            <Breadcrumb>
                <BreadcrumbItem>
                    <Link href=crate::paths::RUNS.to_string()>"Runs"</Link>
                </BreadcrumbItem>
                <Transition fallback=|| ()>
                    {move || run_res.get().and_then(|r| r.ok().flatten()).map(|d| {
                        let task = d.run.task_name;
                        let href = boson_backend::boson_task_path(&task);
                        view! {
                            <BreadcrumbDivider />
                            <BreadcrumbItem>
                                <Link href=href>{task}</Link>
                            </BreadcrumbItem>
                        }
                    })}
                </Transition>
                <BreadcrumbDivider />
                <BreadcrumbItem>
                    <BreadcrumbButton current=true>
                        {move || format!("Run {}", boson_backend::short_id(&run_id.get()))}
                    </BreadcrumbButton>
                </BreadcrumbItem>
            </Breadcrumb>
        </div>
    }
}

/// Detail view for a single run: breadcrumb, status, every attempt of its job, the error,
/// and live updates.
#[component]
pub fn BosonRunDetailPage() -> impl IntoView {
    let params = use_params_map();
    let run_id = Memo::new(move |_| params.get().get("id").unwrap_or_default());

    let run_res = Resource::new(
        move || run_id.get(),
        move |id| async move {
            if id.is_empty() {
                return Ok(None);
            }
            get_run_detail(id).await
        },
    );

    let live = boson_job_run_subscription();

    let job_id_for_live = Memo::new(move |_| {
        run_res
            .get()
            .and_then(|r| r.ok().flatten())
            .filter(|d| d.run.status == RunStatusDto::Running)
            .map(|d| d.run.job_id)
            .filter(|id| !id.is_empty())
    });

    Effect::new(move |_| {
        let _ = live.trigger.get();
        let current_run = run_id.get();
        if current_run.is_empty() {
            return;
        }
        if let Some(ev) = live.latest_event.get() {
            if boson_run_event_is_status(&ev) && boson_run_event_matches_run(&ev, &current_run) {
                run_res.refetch();
            }
        }
    });

    let on_changed = Callback::new(move |()| run_res.refetch());

    view! {
        <BosonJobRunLiveSource
            job_id=Signal::derive(move || job_id_for_live.get())
            trigger=live.trigger
            latest_event=live.latest_event
        />
        <ContentContainer data_testid="boson-run-detail">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <RunBreadcrumb run_id=run_id run_res=run_res />

                <Suspense fallback=move || view! { <RunDetailSkeleton /> }>
                    {move || match run_res.get() {
                        Some(Ok(Some(d))) => {
                            let error_msg = d.run.error_message.clone();
                            let error_show = Signal::from(error_msg.is_some());
                            let attempt_text = attempt_line(&d);
                            let header_task = d.run.task_name.clone();
                            let status = d.run.status;
                            let attempts = d.attempts.clone();
                            let current_run_id = d.run.run_id.clone();
                            let actions_detail = d.clone();
                            view! {
                                <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                                    <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Start gap=SpacingSize::Size160.flex_gap()>
                                        <Flex vertical=true gap=SpacingSize::Size40.flex_gap()>
                                            <Flex align=FlexAlign::Center gap=SpacingSize::Size120.flex_gap()>
                                                <Title3>{header_task}</Title3>
                                                <RunStatusBadge status=status />
                                            </Flex>
                                            <div data-testid="run-detail-attempt-line">
                                                <Caption1>{attempt_text}</Caption1>
                                            </div>
                                        </Flex>
                                        <RunActions detail=actions_detail on_changed=on_changed />
                                    </Flex>
                                    <RunInfoGrid detail=d />
                                    <RunAttempts attempts=attempts current_run_id=current_run_id />
                                    <OrbitalPresence
                                        show=error_show
                                        motion=boson_error_reveal_motion()
                                        appear=true
                                    >
                                        {move || {
                                            error_msg.clone().map(|msg| {
                                                view! { <RunErrorDisplay message=msg /> }
                                            })
                                        }}
                                    </OrbitalPresence>
                                </Flex>
                            }.into_any()
                        }
                        Some(Ok(None)) => view! {
                            <MessageBar intent=MessageBarIntent::Warning>"Run not found"</MessageBar>
                        }.into_any(),
                        Some(Err(e)) => server_fn_error_or_denied(&e, "see this run and its error"),
                        None => view! { <RunDetailSkeleton /> }.into_any(),
                    }}
                </Suspense>
            </Flex>
        </ContentContainer>
    }
}
