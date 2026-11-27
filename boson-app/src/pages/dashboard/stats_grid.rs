use leptos::prelude::*;
use orbital::components::{
    Caption1, Skeleton, SkeletonItem, SkeletonItemSize, SpacingSize, StatCard, StatCardVariant,
};
use orbital::primitives::{
    Card, Flex, FlexAlign, FlexWrap, Icon, InfoLabel, InfoLabelInfo, Link, MessageBar,
    MessageBarIntent,
};

use crate::components::{boson_kpi_enter_motion, failed_24h_help, BosonHelpStatCard};
use crate::server::DashboardStats;
use orbital_motion::{MotionDuration, OrbitalPresenceGroup, OrbitalPresenceGroupItem};

/// Where each KPI card leads.
const WAITING_HREF: &str = "/boson/jobs?tab=active";
const RUNNING_HREF: &str = "/boson/jobs?tab=active";
const FAILED_HREF: &str = "/boson/runs?status=failed";

/// Stat card shell with a skeleton placeholder for the value only.
#[component]
fn DashboardStatCardSkeleton(
    /// Label text.
    label: &'static str,
    /// Icon to display.
    icon: icondata_core::Icon,
    /// Help shown beside the label.
    #[prop(optional)]
    info: Option<AnyView>,
) -> impl IntoView {
    let value_skeleton = Signal::from(SkeletonItemSize::S32);
    let (style_sheet, class_names) = turf::inline_style_sheet_values! {
        .Card { min-width: 140px; flex: 1; }
        .Label { color: var(--orb-color-text-tertiary); }
        .ValueSkeleton { width: 4rem; }
    };
    let label_view = match info {
        Some(info) => view! {
            <InfoLabel>
                <Caption1 class=class_names.label>{label}</Caption1>
                <InfoLabelInfo slot>{info}</InfoLabelInfo>
            </InfoLabel>
        }
        .into_any(),
        None => view! { <Caption1 class=class_names.label>{label}</Caption1> }.into_any(),
    };

    view! {
        <style>{style_sheet}</style>
        <Card class=class_names.card>
            <Flex vertical=true gap=SpacingSize::Size80.flex_gap() padding=SpacingSize::Size160.inset()>
                <Flex align=FlexAlign::Center gap=SpacingSize::Size80.flex_gap()>
                    <Icon icon=icon />
                    {label_view}
                </Flex>
                <Skeleton>
                    <SkeletonItem class=class_names.value_skeleton size=value_skeleton />
                </Skeleton>
            </Flex>
        </Card>
    }
}

/// Skeleton row matching the four dashboard KPI cards.
#[component]
fn DashboardStatsSkeleton() -> impl IntoView {
    view! {
        <Flex gap=SpacingSize::Size160.flex_gap() wrap=FlexWrap::Wrap>
            <div data-testid="dashboard-stat-waiting">
                <DashboardStatCardSkeleton label="Jobs waiting" icon=icondata::AiUnorderedListOutlined />
            </div>
            <div data-testid="dashboard-stat-running">
                <DashboardStatCardSkeleton label="Running" icon=icondata::AiThunderboltOutlined />
            </div>
            <div data-testid="dashboard-stat-failed">
                <DashboardStatCardSkeleton
                    label="Failed (24h)"
                    icon=icondata::AiCloseCircleOutlined
                    info=failed_24h_help()
                />
            </div>
            <div data-testid="dashboard-stat-oldest">
                <DashboardStatCardSkeleton label="Oldest waiting" icon=icondata::AiClockCircleOutlined />
            </div>
        </Flex>
    }
}

/// Loaded KPI cards with staggered enter motion; each card links to the list behind it.
#[component]
fn DashboardStatsCards(
    /// Whether enter motion should show.
    kpi_enter: ReadSignal<bool>,
    /// Loaded stats.
    stats: DashboardStats,
) -> impl IntoView {
    let now = chrono::Utc::now();
    let failed_variant = if stats.failed_24h > 0 {
        StatCardVariant::Danger
    } else {
        StatCardVariant::Default
    };
    let oldest_variant = if boson_backend::oldest_waiting_is_slow(stats.oldest_waiting_at, now) {
        StatCardVariant::Warning
    } else {
        StatCardVariant::Default
    };
    let waiting = stats.jobs_waiting.to_string();
    let running = stats.jobs_running.to_string();
    let failed = stats.failed_24h.to_string();
    let oldest = boson_backend::oldest_waiting_label(stats.oldest_waiting_at, now);

    view! {
        <OrbitalPresenceGroup
            motion=boson_kpi_enter_motion()
            stagger=Signal::from(MotionDuration::Normal)
        >
            <Flex gap=SpacingSize::Size160.flex_gap() wrap=FlexWrap::Wrap>
                <OrbitalPresenceGroupItem show=kpi_enter index=Signal::from(0usize)>
                    <div data-testid="dashboard-stat-waiting">
                        <Link href=WAITING_HREF>
                            <StatCard
                                label="Jobs waiting"
                                value={Signal::<String>::from(waiting)}
                                icon=icondata::AiUnorderedListOutlined
                            />
                        </Link>
                    </div>
                </OrbitalPresenceGroupItem>
                <OrbitalPresenceGroupItem show=kpi_enter index=Signal::from(1usize)>
                    <div data-testid="dashboard-stat-running">
                        <Link href=RUNNING_HREF>
                            <StatCard
                                label="Running"
                                value={Signal::<String>::from(running)}
                                icon=icondata::AiThunderboltOutlined
                            />
                        </Link>
                    </div>
                </OrbitalPresenceGroupItem>
                <OrbitalPresenceGroupItem show=kpi_enter index=Signal::from(2usize)>
                    <div data-testid="dashboard-stat-failed">
                        <Link href=FAILED_HREF>
                            <BosonHelpStatCard
                                label="Failed (24h)"
                                value={Signal::<String>::from(failed)}
                                icon=icondata::AiCloseCircleOutlined
                                variant=failed_variant
                                label_info=failed_24h_help()
                            />
                        </Link>
                    </div>
                </OrbitalPresenceGroupItem>
                <OrbitalPresenceGroupItem show=kpi_enter index=Signal::from(3usize)>
                    <div data-testid="dashboard-stat-oldest">
                        <Link href=boson_backend::BOSON_QUEUES_PATH>
                            <StatCard
                                label="Oldest waiting"
                                value={Signal::<String>::from(oldest)}
                                icon=icondata::AiClockCircleOutlined
                                variant=oldest_variant
                            />
                        </Link>
                    </div>
                </OrbitalPresenceGroupItem>
            </Flex>
        </OrbitalPresenceGroup>
    }
}

/// KPI stat cards with staggered enter on first load; poll refetches keep the old cards
/// until the new numbers land.
#[component]
pub fn DashboardStatsGrid(
    /// Resource that loads the stats data.
    stats_res: Resource<Result<DashboardStats, ServerFnError>>,
) -> impl IntoView {
    let kpi_enter = RwSignal::new(false);

    Effect::new(move |_| {
        if stats_res.get().and_then(Result::ok).is_some() {
            kpi_enter.set(true);
        }
    });

    view! {
        <Transition fallback=move || view! { <DashboardStatsSkeleton /> }>
            {move || stats_res.get().map(|r| match r {
                Ok(stats) => view! {
                    <DashboardStatsCards kpi_enter=kpi_enter.read_only() stats=stats />
                }.into_any(),
                Err(e) => view! {
                    <MessageBar intent=MessageBarIntent::Error>
                        "Couldn't load the numbers. " {e.to_string()}
                    </MessageBar>
                }.into_any(),
            })}
        </Transition>
    }
}
