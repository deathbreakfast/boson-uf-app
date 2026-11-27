use std::collections::BTreeMap;

use leptos::prelude::*;
use orbital_charts::{
    AxisDef, BarChart, ChartItemId, ChartType, GridConfig, LegendConfig, ScaleType, SeriesDef,
};

use crate::server::DashboardChartSeries;

/// Shared stack id so the three outcome series stack in one bar per bucket.
const STACK: &str = "outcomes";

/// True when every bucket in every series is zero (no finished runs in range).
pub fn run_outcome_series_is_empty(series: &[DashboardChartSeries]) -> bool {
    series
        .iter()
        .flat_map(|s| s.points.iter())
        .all(|p| p.value <= 0.0)
}

fn bucket_label(ts: chrono::DateTime<chrono::Utc>, use_daily: bool) -> String {
    if use_daily {
        ts.format("%m/%d").to_string()
    } else {
        // Include date so 24h windows that cross midnight stay unique on the band axis.
        format!("{} {}", ts.format("%m/%d"), ts.format("%H:00"))
    }
}

/// Status colour for each outcome series, from Orbital's status tokens.
fn series_color(id: &str) -> Option<String> {
    let token = match id {
        "succeeded" => "success",
        "failed" => "danger",
        "timed_out_or_canceled" => "warning",
        _ => return None,
    };
    Some(format!("var(--orb-color-status-{token}-fg)"))
}

/// Bucket start timestamps in axis order.
pub fn bucket_starts(series: &[DashboardChartSeries]) -> Vec<chrono::DateTime<chrono::Utc>> {
    let mut starts: Vec<_> = series
        .iter()
        .flat_map(|s| s.points.iter().map(|p| p.ts))
        .collect();
    starts.sort();
    starts.dedup();
    starts
}

/// Stacked bar chart of run outcomes; clicking a bar calls `on_bucket` with its index.
pub fn bar_chart_from_series(
    series: &[DashboardChartSeries],
    height: f64,
    use_daily_labels: bool,
    on_bucket: Callback<(usize,), ()>,
) -> impl IntoView {
    let ts_order = bucket_starts(series);
    let categories: Vec<String> = ts_order
        .iter()
        .map(|ts| bucket_label(*ts, use_daily_labels))
        .collect();

    let chart_series: Vec<SeriesDef> = series
        .iter()
        .map(|s| {
            let map: BTreeMap<_, _> = s.points.iter().map(|p| (p.ts, p.value)).collect();
            let data: Vec<f64> = ts_order
                .iter()
                .map(|ts| map.get(ts).copied().unwrap_or(0.0))
                .collect();
            SeriesDef {
                id: s.id.clone(),
                label: Some(s.label.clone()),
                data: Some(data),
                chart_type: Some(ChartType::Bar),
                stack_group: Some(STACK.to_string()),
                color: series_color(&s.id),
                ..Default::default()
            }
        })
        .collect();

    let x_axis = vec![AxisDef {
        id: "x".into(),
        scale_type: ScaleType::Band,
        data: Some(categories),
        ..Default::default()
    }];

    let on_item_click = Callback::new(move |(item,): (ChartItemId,)| {
        on_bucket.run((item.data_index,));
    });

    view! {
        <BarChart
            series=chart_series
            x_axis=x_axis
            height=height
            grid=GridConfig {
                horizontal: true,
                vertical: false,
            }
            legend=LegendConfig::default()
            skip_animation=true
            on_item_click=on_item_click
        />
    }
}
