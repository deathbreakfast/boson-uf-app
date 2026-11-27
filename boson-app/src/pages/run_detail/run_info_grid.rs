use leptos::prelude::*;
use orbital::components::{Body1, Caption2, Card};
use orbital::primitives::{Grid, GridConfig, GridItem, InfoLabel, InfoLabelInfo};

use crate::components::{duration_help, BosonCardContent, BosonTableLink};
use crate::server::RunDetail;

/// Metadata grid for a run: task, job and queue as links, who started it, times, and the full
/// run id last.
#[component]
pub fn RunInfoGrid(
    /// Run detail to display.
    detail: RunDetail,
) -> impl IntoView {
    let run = detail.run;
    let task_href = boson_backend::boson_task_path(&run.task_name);
    let job_href = boson_backend::boson_runs_job_filter_path(&run.job_id);
    let job_label = detail
        .job
        .as_ref()
        .map_or_else(|| boson_backend::short_id(&run.job_id), |j| j.label.clone());
    let queue = detail.job.as_ref().map(|j| j.pool.clone());
    let started_by = detail.job.as_ref().map(|j| j.started_by.clone());
    let duration = run
        .duration_ms
        .map_or_else(|| "-".to_string(), boson_backend::format_duration_ms);

    let (style_sheet, class_names) = turf::inline_style_sheet_values! {
        .Label { color: var(--orb-color-text-tertiary); }
    };

    view! {
        <style>{style_sheet}</style>
        <Card>
            <BosonCardContent>
                <div data-testid="run-info-grid">
                <Grid config=GridConfig::with_gaps(2, 24, 8)>
                    <GridItem><Caption2 class=class_names.label>"Task"</Caption2></GridItem>
                    <GridItem>
                        <span data-testid="run-info-task">
                            <BosonTableLink href=task_href>{run.task_name.clone()}</BosonTableLink>
                        </span>
                    </GridItem>

                    <GridItem><Caption2 class=class_names.label>"Job"</Caption2></GridItem>
                    <GridItem>
                        <span data-testid="run-info-job">
                            <BosonTableLink href=job_href>{job_label}</BosonTableLink>
                        </span>
                    </GridItem>

                    {queue.map(|pool| {
                        let href = boson_backend::boson_queue_pool_path(&pool);
                        view! {
                            <GridItem><Caption2 class=class_names.label>"Queue"</Caption2></GridItem>
                            <GridItem>
                                <span data-testid="run-info-queue">
                                    <BosonTableLink href=href>{pool}</BosonTableLink>
                                </span>
                            </GridItem>
                        }
                    })}

                    {started_by.map(|who| view! {
                        <GridItem><Caption2 class=class_names.label>"Started by"</Caption2></GridItem>
                        <GridItem><Body1 attr:data-testid="run-info-started-by">{who}</Body1></GridItem>
                    })}

                    <GridItem><Caption2 class=class_names.label>"Started"</Caption2></GridItem>
                    <GridItem><Body1>{run.started_at.clone()}</Body1></GridItem>

                    <GridItem><Caption2 class=class_names.label>"Finished"</Caption2></GridItem>
                    <GridItem><Body1>{run.finished_at.clone().unwrap_or_else(|| "-".to_string())}</Body1></GridItem>

                    <GridItem>
                        <InfoLabel>
                            <Caption2 class=class_names.label>"Duration"</Caption2>
                            <InfoLabelInfo slot>
                                {duration_help()}
                            </InfoLabelInfo>
                        </InfoLabel>
                    </GridItem>
                    <GridItem><Body1>{duration}</Body1></GridItem>

                    <GridItem><Caption2 class=class_names.label>"Run id"</Caption2></GridItem>
                    <GridItem><Caption2>{run.run_id.clone()}</Caption2></GridItem>
                </Grid>
                </div>
            </BosonCardContent>
        </Card>
    }
}
