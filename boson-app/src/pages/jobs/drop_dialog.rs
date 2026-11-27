//! Confirm before dropping failed jobs: they leave the failed list for good; runs stay.

use leptos::prelude::*;
use orbital::components::{
    Body1, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle,
};
use orbital::primitives::{Button, ButtonAppearance};

use crate::server::JobSummary;

use super::copy::{drop_body, drop_title};

/// Drop confirm dialog with a Danger button, since a drop can't be undone.
#[component]
pub fn DropJobsDialog(
    /// Whether the dialog shows.
    open: RwSignal<bool>,
    /// The failed jobs to drop.
    #[prop(into)]
    jobs: Signal<Vec<JobSummary>>,
    /// True while the drop is in flight.
    #[prop(into)]
    pending: Signal<bool>,
    /// Called when Drop is pressed.
    on_confirm: Callback<()>,
) -> impl IntoView {
    view! {
        <Dialog open=open>
            <DialogSurface>
                <DialogBody>
                    <DialogTitle>{move || drop_title(jobs.with(Vec::len))}</DialogTitle>
                    <DialogContent>
                        <div data-testid="drop-jobs-dialog">
                            <Body1>{move || jobs.with(|j| drop_body(j))}</Body1>
                        </div>
                    </DialogContent>
                    <DialogActions>
                        <Button
                            appearance=ButtonAppearance::Secondary
                            on_click=Callback::new(move |_| open.set(false))
                        >
                            "Cancel"
                        </Button>
                        <span data-testid="drop-jobs-confirm">
                            <Button
                                appearance=ButtonAppearance::Danger
                                disabled=pending
                                on_click=Callback::new(move |_| on_confirm.run(()))
                            >
                                {move || match jobs.with(Vec::len) {
                                    1 => "Drop job".to_string(),
                                    n => format!("Drop {n} jobs"),
                                }}
                            </Button>
                        </span>
                    </DialogActions>
                </DialogBody>
            </DialogSurface>
        </Dialog>
    }
}
