//! Jobs: what's waiting and running (Active), what used every attempt (Failed), and what's
//! done (Finished). `/boson/queue` redirects here.

mod cancel_dialog;
mod copy;
mod drop_dialog;
mod failed;
mod retry_dialog;

use std::collections::{HashMap, HashSet};

use boson_backend::JobsTab;
use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
#[cfg(feature = "ssr")]
use leptos_router::components::Redirect;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_router::NavigateOptions;
use orbital::components::{
    Card, ContentContainer, MessageBar, MessageBarIntent, SpacingSize, Tab, TabList, Title3,
};
use orbital::primitives::Flex;

use crate::components::{
    boson_table_page_layout, use_show_mode, BosonCardContent, JobsDataTable, ListHeader, OwnedKind,
};
use crate::errors::{friendly_error, FriendlyError, FriendlyErrorBar};
use crate::live::{boson_job_event_is_status, boson_jobs_subscription, BosonJobsLiveSource};
use crate::server::{cancel_job, JobSummary};

use cancel_dialog::CancelJobDialog;
use copy::cancel_success;
use failed::FailedJobsPanel;

/// `/boson/queue`, the page's old address: sends people to Jobs.
///
/// The server render asks for a redirect (hosts that honor it answer with `Location`), and
/// the browser replaces the URL once the page hydrates, so the old address works either way.
#[component]
pub fn BosonQueueRedirect() -> impl IntoView {
    let navigate = use_navigate();
    Effect::new(move |_| {
        navigate(
            crate::paths::JOBS,
            NavigateOptions {
                replace: true,
                ..NavigateOptions::default()
            },
        );
    });
    #[cfg(feature = "ssr")]
    {
        view! { <Redirect path=crate::paths::JOBS /> }.into_any()
    }
    #[cfg(not(feature = "ssr"))]
    {
        ().into_any()
    }
}

/// The tab in the URL (`?tab=`), and the tab list's selection kept in step with it both ways.
fn use_jobs_tab() -> (Memo<JobsTab>, RwSignal<String>) {
    let query = use_query_map();
    let navigate = use_navigate();
    let navigate_store = StoredValue::new(navigate);

    let tab =
        Memo::new(move |_| JobsTab::from_key(&query.with(|q| q.get("tab").unwrap_or_default())));
    let selected_tab = RwSignal::new(tab.get_untracked().key().to_string());

    // URL → tab list (back/forward), and tab list → URL.
    Effect::new(move |_| {
        let key = tab.get().key().to_string();
        if selected_tab.get_untracked() != key {
            selected_tab.set(key);
        }
    });
    Effect::new(move |_| {
        let picked = JobsTab::from_key(&selected_tab.get());
        if picked != tab.get_untracked() {
            let href = if picked == JobsTab::Active {
                crate::paths::JOBS.to_string()
            } else {
                format!("{}?tab={}", crate::paths::JOBS, picked.key())
            };
            navigate_store.with_value(|nav| {
                nav(
                    &href,
                    NavigateOptions {
                        replace: true,
                        scroll: false,
                        ..NavigateOptions::default()
                    },
                );
            });
        }
    });

    (tab, selected_tab)
}

/// State and handlers for Cancel: the confirm dialog, the call, and its message.
#[derive(Clone, Copy)]
struct CancelFlow {
    pending: RwSignal<HashSet<String>>,
    open: RwSignal<bool>,
    job_id: RwSignal<Option<String>>,
    job: Signal<Option<JobSummary>>,
    success: RwSignal<Option<String>>,
    error: RwSignal<Option<FriendlyError>>,
    on_cancel: Callback<String>,
    on_confirm: Callback<()>,
}

fn cancel_flow(rows: RwSignal<HashMap<String, JobSummary>>, refresh: RwSignal<u32>) -> CancelFlow {
    let cancel_pending = RwSignal::new(HashSet::<String>::new());
    let cancel_open = RwSignal::new(false);
    let cancel_id = RwSignal::new(None::<String>);
    let cancel_success_message = RwSignal::new(None::<String>);
    let cancel_error = RwSignal::new(None::<FriendlyError>);
    let cancel_job_row = Signal::derive(move || {
        let id = cancel_id.get()?;
        rows.with(|map| map.get(&id).cloned())
    });

    let on_cancel = Callback::new(move |job_id: String| {
        if cancel_pending.get_untracked().contains(&job_id) {
            return;
        }
        cancel_id.set(Some(job_id));
        cancel_open.set(true);
    });

    let on_confirm_cancel = Callback::new(move |()| {
        let Some(job_id) = cancel_id.get_untracked() else {
            return;
        };
        let job = cancel_job_row.get_untracked();
        cancel_open.set(false);
        cancel_error.set(None);
        cancel_success_message.set(None);
        cancel_pending.update(|s| {
            s.insert(job_id.clone());
        });
        spawn_local_scoped(async move {
            match cancel_job(job_id.clone()).await {
                Ok(()) => cancel_success_message.set(Some(cancel_success(&job_id, job.as_ref()))),
                Err(e) => cancel_error.set(Some(friendly_error(&e))),
            }
            cancel_pending.update(|s| {
                s.remove(&job_id);
            });
            refresh.update(|n| *n += 1);
        });
    });

    CancelFlow {
        pending: cancel_pending,
        open: cancel_open,
        job_id: cancel_id,
        job: cancel_job_row,
        success: cancel_success_message,
        error: cancel_error,
        on_cancel,
        on_confirm: on_confirm_cancel,
    }
}

/// Jobs page: tabs for Active (the default), Failed and Finished, kept in `?tab=`.
///
/// Switching tabs swaps only the card below the tabs; the title, tabs and messages stay.
#[component]
pub fn BosonJobsPage() -> impl IntoView {
    let (tab, selected_tab) = use_jobs_tab();
    let (show, on_show_change) = use_show_mode();

    let rows = RwSignal::new(HashMap::<String, JobSummary>::new());
    let refresh = RwSignal::new(0u32);
    let refresh_signal = Signal::derive(move || refresh.get());
    let live = boson_jobs_subscription();

    Effect::new(move |_| {
        let _ = live.trigger.get();
        let Some(ev) = live.latest_event.get() else {
            return;
        };
        if boson_job_event_is_status(&ev) {
            refresh.update(|n| *n += 1);
        }
    });

    let CancelFlow {
        pending: cancel_pending,
        open: cancel_open,
        job_id: cancel_id,
        job: cancel_job_row,
        success: cancel_success_message,
        error: cancel_error,
        on_cancel,
        on_confirm: on_confirm_cancel,
    } = cancel_flow(rows, refresh);

    let (page_style, page_classes) = boson_table_page_layout();
    let card_class = StoredValue::new(page_classes.card.clone());
    let card_content_class = StoredValue::new(page_classes.card_content.clone());

    view! {
        <style>{page_style}</style>
        <BosonJobsLiveSource trigger=live.trigger latest_event=live.latest_event />
        <ContentContainer class=page_classes.page data_testid="boson-jobs">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap() class=page_classes.body>
                <Title3>"Jobs"</Title3>
                <div data-testid="jobs-tabs">
                    <TabList selected_value=selected_tab>
                        <Tab value="active">"Active"</Tab>
                        <Tab value="failed">"Failed"</Tab>
                        <Tab value="finished">"Finished"</Tab>
                    </TabList>
                </div>
                {move || cancel_success_message.get().map(|message| view! {
                    <div data-testid="jobs-cancel-success">
                        <MessageBar intent=MessageBarIntent::Success>{message}</MessageBar>
                    </div>
                })}
                {move || cancel_error.get().map(|e| view! {
                    <FriendlyErrorBar error=e data_testid="jobs-cancel-error" />
                })}
                <ListHeader kind=OwnedKind::TaskTypes mode=show on_change=on_show_change />

                // Rebuilt when the tab or Show changes (a new list from its first page).
                {move || match (tab.get(), show.get()) {
                    (JobsTab::Failed, mode) => view! {
                        <FailedJobsPanel
                            rows=rows
                            refresh=refresh
                            card_class=card_class.get_value()
                            card_content_class=card_content_class.get_value()
                            show=mode
                        />
                    }
                    .into_any(),
                    (active_or_finished, mode) => view! {
                        <Card class=card_class.get_value()>
                            <BosonCardContent class=card_content_class.get_value()>
                                <JobsDataTable
                                    tab=active_or_finished
                                    rows=rows
                                    cancel_pending=cancel_pending.read_only()
                                    on_cancel=on_cancel
                                    refresh_signal=refresh_signal
                                    show=mode
                                />
                            </BosonCardContent>
                        </Card>
                    }
                    .into_any(),
                }}
            </Flex>
            <CancelJobDialog
                open=cancel_open
                job_id=cancel_id
                job=cancel_job_row
                on_confirm=on_confirm_cancel
            />
        </ContentContainer>
    }
}
