use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use orbital::components::{
    Button, Caption1, Card, Flex, FlexGap, FlexWrap, MessageBar, MessageBarIntent, Skeleton,
    SkeletonItem, SpacingSize,
};
use orbital::primitives::{ButtonAppearance, FlexJustify, Link};

use super::charts::{bar_chart_from_series, bucket_starts, run_outcome_series_is_empty};
use crate::components::{run_outcomes_chart_help, BosonCardContent, BosonHelpCardHeader};
use crate::server::get_run_stats_series;

const RANGE_24H: i64 = 86_400;
const RANGE_7D: i64 = 604_800;

#[component]
fn RunTrendChartSkeleton(
    /// Additional CSS class(es) to apply.
    #[prop(into)]
    class: String,
) -> impl IntoView {
    view! {
        <Skeleton>
            <SkeletonItem class=class />
        </Skeleton>
    }
}

/// Stacked outcome bars; picking a bar opens Runs for that bucket's `from` and `to`.
#[component]
fn RunTrendChart(
    /// Loaded series.
    series: Vec<crate::server::DashboardChartSeries>,
    /// Range the series cover, in seconds.
    range: i64,
) -> impl IntoView {
    let starts = bucket_starts(&series);
    let step = boson_backend::run_bucket_step(boson_backend::run_bucket_granularity(range));
    let navigate = use_navigate();
    let on_bucket = Callback::new(move |(i,): (usize,)| {
        if let Some(from) = starts.get(i) {
            navigate(
                &boson_backend::boson_runs_window_path(*from, *from + step),
                NavigateOptions::default(),
            );
        }
    });
    bar_chart_from_series(&series, 280.0, range > RANGE_24H, on_bucket)
}

#[component]
pub fn BosonRunTrendCard() -> impl IntoView {
    let range_secs = RwSignal::new(RANGE_24H);
    let res = Resource::new(
        move || range_secs.get(),
        |secs| async move { get_run_stats_series(secs).await },
    );

    let (style_sheet, class_names) = turf::inline_style_sheet_values! {
        .ChartSkeleton { width: 100%; height: 280px; }
    };
    let chart_skeleton_class = StoredValue::new(class_names.chart_skeleton.to_string());

    view! {
        <style>{style_sheet}</style>
        <Card>
            <BosonHelpCardHeader
                title="Run outcomes"
                info=run_outcomes_chart_help()
            />
            <BosonCardContent>
                <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                    <Flex gap=FlexGap::Small wrap=FlexWrap::Wrap>
                        <div data-testid="dashboard-run-trend-range-24h">
                            <Button
                                appearance=Signal::derive(move || {
                                    if range_secs.get() == RANGE_24H {
                                        ButtonAppearance::Primary
                                    } else {
                                        ButtonAppearance::Secondary
                                    }
                                })
                                on:click=move |_| range_secs.set(RANGE_24H)
                            >
                                "24h"
                            </Button>
                        </div>
                        <div data-testid="dashboard-run-trend-range-7d">
                            <Button
                                appearance=Signal::derive(move || {
                                    if range_secs.get() == RANGE_7D {
                                        ButtonAppearance::Primary
                                    } else {
                                        ButtonAppearance::Secondary
                                    }
                                })
                                on:click=move |_| range_secs.set(RANGE_7D)
                            >
                                "7d"
                            </Button>
                        </div>
                    </Flex>

                    <div data-testid="dashboard-run-trend-chart">
                        <Show
                            when=move || res.get().is_some()
                            fallback=move || view! {
                                <RunTrendChartSkeleton class=chart_skeleton_class.with_value(Clone::clone) />
                            }
                        >
                            <Transition fallback=move || view! {
                                <RunTrendChartSkeleton class=chart_skeleton_class.with_value(Clone::clone) />
                            }>
                                {move || res.get().map(|r| match r {
                                    Ok(series) if run_outcome_series_is_empty(&series) => view! {
                                        <MessageBar intent=MessageBarIntent::Info>
                                            "No runs in this time range."
                                        </MessageBar>
                                    }.into_any(),
                                    Ok(series) => view! {
                                        <RunTrendChart series=series range=range_secs.get_untracked() />
                                    }.into_any(),
                                    Err(e) => view! {
                                        <MessageBar intent=MessageBarIntent::Error>
                                            {format!("Couldn't load the chart. {}", crate::errors::friendly_error(&e).sentence())}
                                        </MessageBar>
                                    }.into_any(),
                                })}
                            </Transition>
                        </Show>
                    </div>

                    <Flex justify=FlexJustify::SpaceBetween wrap=FlexWrap::Wrap gap=FlexGap::Small>
                        <Caption1>"Pick a bar to see its runs."</Caption1>
                        <div data-testid="dashboard-run-trend-view-all">
                            <Link href=crate::paths::RUNS>
                                <Caption1>"All runs"</Caption1>
                            </Link>
                        </div>
                    </Flex>
                </Flex>
            </BosonCardContent>
        </Card>
    }
}
