//! Queues: one row per Gluon pool with its state and last-hour numbers, and a detail page
//! with pause, resume and purge.

mod detail;
mod purge_dialog;

pub use detail::BosonQueueDetailPage;

use std::sync::Arc;

use leptos::prelude::*;
use orbital::components::{Card, ContentContainer, EmptyState, SpacingSize, Subtitle2, Title3};
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, DataRecord, DataTable, DataTableColumnDef,
    DataTableEmptyView, DataTableFeatures, DataTableLoadingView, DataTableRowModel,
    DataTableSource, Flex, Link, PagingMode,
};

use crate::components::{
    no_toolbar, owner_column, row_model, text_cell, use_show_mode, AccessBadge, BosonCardContent,
    BosonDataTableRefetchSkeleton, ListHeader, OwnedKind, TableLoadError,
};
use crate::errors::friendly_error;
use crate::live::use_boson_poll_tick;
use crate::permissions::NO_ACCESS;
use crate::server::{list_queues, QueueStateDto, QueueSummary};

/// Badge for a queue's state: Falling behind (warning), Keeping up (success), Paused.
#[component]
pub fn QueueStateBadge(
    /// Current state.
    state: QueueStateDto,
) -> impl IntoView {
    let color = match state {
        QueueStateDto::FallingBehind => BadgeColor::Warning,
        QueueStateDto::KeepingUp => BadgeColor::Success,
        QueueStateDto::Paused => BadgeColor::Informative,
    };
    view! {
        <Badge appearance=BadgeAppearance::Tint color=color>{state.label()}</Badge>
    }
}

const fn state_key(state: QueueStateDto) -> &'static str {
    match state {
        QueueStateDto::FallingBehind => "falling_behind",
        QueueStateDto::KeepingUp => "keeping_up",
        QueueStateDto::Paused => "paused",
    }
}

fn state_from_key(key: &str) -> QueueStateDto {
    match key {
        "falling_behind" => QueueStateDto::FallingBehind,
        "paused" => QueueStateDto::Paused,
        _ => QueueStateDto::KeepingUp,
    }
}

fn columns() -> Vec<DataTableColumnDef> {
    let name_view = Arc::new(|record: DataRecord| {
        let name = text_cell(&record, "name");
        let href = boson_backend::boson_queue_path(&name);
        let testid = format!("queue-row-{name}");
        view! { <Link href=href attr:data-testid=testid>{name}</Link> }.into_any()
    });
    let state_view = Arc::new(|record: DataRecord| {
        if text_cell(&record, "access") == NO_ACCESS {
            return ().into_any();
        }
        let state = state_from_key(&text_cell(&record, "state"));
        let testid = format!("queue-state-{}", text_cell(&record, "name"));
        view! { <span data-testid=testid><QueueStateBadge state=state /></span> }.into_any()
    });
    let access_view = Arc::new(|record: DataRecord| {
        view! { <AccessBadge access=text_cell(&record, "access") /> }.into_any()
    });
    vec![
        DataTableColumnDef::new("name", "Queue")
            .with_sortable(false)
            .with_cell_view(name_view),
        owner_column(),
        DataTableColumnDef::new("state", "State")
            .with_sortable(false)
            .with_cell_view(state_view),
        DataTableColumnDef::new("waiting", "Waiting").with_sortable(false),
        DataTableColumnDef::new("running", "Running").with_sortable(false),
        DataTableColumnDef::new("oldest", "Oldest waiting").with_sortable(false),
        DataTableColumnDef::new("done", "Done last hour").with_sortable(false),
        DataTableColumnDef::new("added", "Added last hour").with_sortable(false),
        DataTableColumnDef::new("access", "Your access")
            .with_sortable(false)
            .with_cell_view(access_view),
    ]
}

/// Queues rows. A queue the reader can't View shows its name, owner and a "No access" tag,
/// with the state and number cells left blank.
fn rows(queues: &[QueueSummary]) -> Vec<DataTableRowModel> {
    let now = chrono::Utc::now();
    queues
        .iter()
        .map(|q| {
            let numbers = q.access != NO_ACCESS;
            let number = |text: String| if numbers { text } else { String::new() };
            row_model(
                &q.name,
                &[
                    ("name", q.name.clone()),
                    ("owner", q.owner.clone()),
                    ("yours", if q.yours { "yes" } else { "" }.to_string()),
                    ("state", state_key(q.state).to_string()),
                    ("waiting", number(q.waiting.to_string())),
                    ("running", number(q.running.to_string())),
                    (
                        "oldest",
                        number(boson_backend::oldest_waiting_label(
                            q.oldest_waiting_at,
                            now,
                        )),
                    ),
                    ("done", number(q.done_last_hour.to_string())),
                    ("added", number(q.added_last_hour.to_string())),
                    ("access", q.access.clone()),
                ],
            )
        })
        .collect()
}

#[component]
fn QueuesTable(
    /// Rows to show.
    queues: Vec<QueueSummary>,
    /// True while the first load is pending.
    loading: bool,
) -> impl IntoView {
    let items = RwSignal::new(rows(&queues));
    let loading = RwSignal::new(loading);
    view! {
        <DataTable
            data_source=DataTableSource::Client(items)
            paging=PagingMode::None
            features=DataTableFeatures::empty()
            toolbar_config=no_toolbar()
            sortable=false
            columns=columns()
            loading=loading
        >
            <DataTableEmptyView slot>
                <EmptyState
                    message="No queues yet"
                    description="A queue appears here once a task routes jobs to its pool."
                />
            </DataTableEmptyView>
            <DataTableLoadingView slot>
                <BosonDataTableRefetchSkeleton />
            </DataTableLoadingView>
        </DataTable>
    }
}

/// `/boson/queues`: whether each pool keeps up.
#[component]
pub fn BosonQueuesPage() -> impl IntoView {
    let poll_tick = use_boson_poll_tick();
    let (show, on_show_change) = use_show_mode();
    let queues_res = Resource::new(
        move || show.get(),
        |mode| async move { list_queues(Some(mode.key().to_string())).await },
    );

    Effect::new(move |_| {
        if poll_tick.get() > 0 {
            queues_res.refetch();
        }
    });

    view! {
        <ContentContainer data_testid="boson-queues">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                    <Title3>"Queues"</Title3>
                    <Subtitle2>
                        "Each queue is a Gluon pool. Falling behind means jobs arrive faster than they finish."
                    </Subtitle2>
                </Flex>
                <ListHeader kind=OwnedKind::Queues mode=show on_change=on_show_change />
                <Card>
                    <BosonCardContent>
                        <div data-testid="boson-queues-table">
                            <Transition fallback=move || view! { <QueuesTable queues=Vec::new() loading=true /> }>
                                {move || queues_res.get().map(|r| match r {
                                    Ok(queues) => view! { <QueuesTable queues=queues loading=false /> }.into_any(),
                                    Err(e) => view! {
                                        <TableLoadError
                                            error=RwSignal::new(Some(friendly_error(&e)))
                                            noun="queues"
                                            on_retry=Callback::new(move |()| queues_res.refetch())
                                        >
                                            ""
                                        </TableLoadError>
                                    }.into_any(),
                                })}
                            </Transition>
                        </div>
                    </BosonCardContent>
                </Card>
            </Flex>
        </ContentContainer>
    }
}
