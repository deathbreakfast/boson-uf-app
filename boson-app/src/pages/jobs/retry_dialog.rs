//! Confirm before retrying failed jobs: from attempt 1, today's retry policy, runs as you.

use leptos::prelude::*;
use orbital::components::{
    Body1, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle,
    MessageBar, MessageBarIntent, SpacingSize,
};
use orbital::primitives::{Button, ButtonAppearance, Flex};

use crate::server::JobSummary;

use super::copy::{retry_body, retry_title, retry_warning};

/// Retry confirm dialog; warns when the last error may repeat.
#[component]
pub fn RetryJobsDialog(
    /// Whether the dialog shows.
    open: RwSignal<bool>,
    /// The failed jobs to retry.
    #[prop(into)]
    jobs: Signal<Vec<JobSummary>>,
    /// True while the retry is in flight.
    #[prop(into)]
    pending: Signal<bool>,
    /// Called when Retry is pressed.
    on_confirm: Callback<()>,
) -> impl IntoView {
    view! {
        <Dialog open=open>
            <DialogSurface>
                <DialogBody>
                    <DialogTitle>{move || retry_title(jobs.with(Vec::len))}</DialogTitle>
                    <DialogContent>
                        <div data-testid="retry-jobs-dialog">
                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                <Body1>{move || jobs.with(|j| retry_body(j))}</Body1>
                                {move || jobs.with(|j| retry_warning(j)).map(|warning| view! {
                                    <MessageBar intent=MessageBarIntent::Warning>{warning}</MessageBar>
                                })}
                            </Flex>
                        </div>
                    </DialogContent>
                    <DialogActions>
                        <Button
                            appearance=ButtonAppearance::Secondary
                            on_click=Callback::new(move |_| open.set(false))
                        >
                            "Cancel"
                        </Button>
                        <span data-testid="retry-jobs-confirm">
                            <Button
                                appearance=ButtonAppearance::Primary
                                disabled=pending
                                on_click=Callback::new(move |_| on_confirm.run(()))
                            >
                                {move || match jobs.with(Vec::len) {
                                    1 => "Retry".to_string(),
                                    n => format!("Retry {n}"),
                                }}
                            </Button>
                        </span>
                    </DialogActions>
                </DialogBody>
            </DialogSurface>
        </Dialog>
    }
}
