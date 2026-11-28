use leptos::prelude::*;
use leptos::task::spawn_local_scoped;
use orbital::components::{
    Body1, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Field,
};
use orbital::primitives::{
    Button, ButtonAppearance, Flex, FlexGap, Input, MessageBar, MessageBarIntent,
};

use crate::server::{purge_queue, QueueSummary};

fn purge_sentence(summary: &QueueSummary) -> String {
    let waiting = match summary.waiting {
        1 => format!(
            "1 waiting job in {} is removed and won't run.",
            summary.name
        ),
        n => format!(
            "{n} waiting jobs in {} are removed and won't run.",
            summary.name
        ),
    };
    let running = match summary.running {
        0 => String::new(),
        1 => " The 1 running job finishes.".to_string(),
        n => format!(" The {n} running jobs finish."),
    };
    format!("{waiting}{running} Whatever enqueued them isn't told.")
}

/// Purge confirm: type the queue's name, see how many jobs go, and that running ones finish.
#[component]
pub fn PurgeDialog(
    /// Open state.
    open: RwSignal<bool>,
    /// Queue being purged, with its current counts.
    summary: QueueSummary,
    /// Called after a purge succeeds.
    on_purged: Callback<()>,
) -> impl IntoView {
    let typed = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let name = StoredValue::new(summary.name.clone());
    let sentence = purge_sentence(&summary);
    let confirm_label = match summary.waiting {
        1 => "Purge 1 job".to_string(),
        n => format!("Purge {n} jobs"),
    };
    let matches = Signal::derive(move || name.with_value(|n| typed.get().trim() == n));

    let on_confirm = move |_| {
        if !matches.get_untracked() || busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(None);
        let queue = name.get_value();
        spawn_local_scoped(async move {
            match purge_queue(queue).await {
                Ok(_) => {
                    typed.set(String::new());
                    open.set(false);
                    on_purged.run(());
                }
                Err(e) => error.set(Some(crate::errors::friendly_error(&e).sentence())),
            }
            busy.set(false);
        });
    };

    view! {
        <Dialog open=open>
            <DialogSurface>
                <DialogBody>
                    <DialogTitle>{format!("Purge waiting jobs in {}?", summary.name)}</DialogTitle>
                    <DialogContent>
                        <Flex vertical=true gap=FlexGap::Medium>
                            <span data-testid="queue-purge-sentence"><Body1>{sentence}</Body1></span>
                            <Field label=format!("Type {} to confirm", summary.name)>
                                <span data-testid="queue-purge-confirm-input">
                                    <Input bind=typed />
                                </span>
                            </Field>
                            {move || error.get().map(|e| view! {
                                <MessageBar intent=MessageBarIntent::Error>{e}</MessageBar>
                            })}
                        </Flex>
                    </DialogContent>
                    <DialogActions>
                        <Button
                            appearance=ButtonAppearance::Secondary
                            on:click=move |_| open.set(false)
                        >
                            "Cancel"
                        </Button>
                        <span data-testid="queue-purge-confirm">
                            <Button
                                appearance=ButtonAppearance::Danger
                                disabled=Signal::derive(move || !matches.get() || busy.get())
                                on:click=on_confirm
                            >
                                {confirm_label}
                            </Button>
                        </span>
                    </DialogActions>
                </DialogBody>
            </DialogSurface>
        </Dialog>
    }
}
