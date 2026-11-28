mod charts;
mod failing_most;
mod needs_attention;
mod run_trend_card;
mod stats_grid;

pub use failing_most::FailingMostCard;
pub use needs_attention::NeedsAttentionCard;
pub use run_trend_card::BosonRunTrendCard;
pub use stats_grid::DashboardStatsGrid;

use leptos::prelude::*;
use orbital::components::{ContentContainer, SpacingSize, Subtitle2, Title3};
use orbital::primitives::Flex;

use crate::live::use_boson_poll_tick;
use crate::server::{get_dashboard_stats, get_failing_most, get_needs_attention};

/// Boson dashboard: what's stuck, what's failing, and whether each queue keeps up.
#[component]
pub fn BosonRootPage() -> impl IntoView {
    let poll_tick = use_boson_poll_tick();
    let stats_res = Resource::new(|| (), |()| async move { get_dashboard_stats().await });
    let attention_res = Resource::new(|| (), |()| async move { get_needs_attention().await });
    let failing_res = Resource::new(|| (), |()| async move { get_failing_most().await });

    Effect::new(move |_| {
        if poll_tick.get() > 0 {
            stats_res.refetch();
            attention_res.refetch();
        }
    });

    view! {
        <ContentContainer data_testid="boson-dashboard">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <Flex vertical=true gap=SpacingSize::Size80.flex_gap()>
                    <Title3>"Dashboard"</Title3>
                    <Subtitle2>"What's stuck, what's failing, and whether each queue is keeping up."</Subtitle2>
                </Flex>

                <DashboardStatsGrid stats_res=stats_res />

                <NeedsAttentionCard lines_res=attention_res />

                <BosonRunTrendCard />

                <FailingMostCard rows_res=failing_res />
            </Flex>
        </ContentContainer>
    }
}
