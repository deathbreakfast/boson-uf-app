//! Confirm before canceling a job: what it is, and that it stops now and won't retry.

use leptos::prelude::*;
use orbital::components::{
    Body1, Caption1, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle,
    SpacingSize,
};
use orbital::primitives::{Button, ButtonAppearance, Flex};

use crate::server::JobSummary;

use super::copy::cancel_body;

/// Cancel confirm dialog with Keep running and Cancel job.
#[component]
pub fn CancelJobDialog(
    /// Whether the dialog shows.
    open: RwSignal<bool>,
    /// Id of the job to cancel.
    #[prop(into)]
    job_id: Signal<Option<String>>,
    /// The job's row, when the table loaded it.
    #[prop(into)]
    job: Signal<Option<JobSummary>>,
    /// Called when Cancel job is pressed.
    on_confirm: Callback<()>,
) -> impl IntoView {
    view! {
        <Dialog open=open>
            <DialogSurface>
                <DialogBody>
                    <DialogTitle>"Cancel this job?"</DialogTitle>
                    <DialogContent>
                        <div data-testid="cancel-job-dialog">
                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                <Body1>{move || cancel_body(job.get().as_ref())}</Body1>
                                <Caption1>
                                    {move || format!("Job {}", job_id.get().unwrap_or_default())}
                                </Caption1>
                            </Flex>
                        </div>
                    </DialogContent>
                    <DialogActions>
                        <Button
                            appearance=ButtonAppearance::Secondary
                            on_click=Callback::new(move |_| open.set(false))
                        >
                            "Keep running"
                        </Button>
                        <span data-testid="cancel-job-confirm">
                            <Button
                                appearance=ButtonAppearance::Primary
                                on_click=Callback::new(move |_| on_confirm.run(()))
                            >
                                "Cancel job"
                            </Button>
                        </span>
                    </DialogActions>
                </DialogBody>
            </DialogSurface>
        </Dialog>
    }
}
