use leptos::prelude::*;
use orbital::components::{Caption1, Card, FormHint, SpacingSize};
use orbital::primitives::{Field, Flex, MessageBar, MessageBarIntent, ProgressBar, Select};

use crate::components::BosonHelpCardHeader;

use super::form_state::{AT_LEAST_ONCE, EXACTLY_ONCE};
use super::SettingsNumberField;

/// "Limits" card: most at once (with how many there are now), enqueues per second, and
/// duplicate handling for idempotency keys.
#[component]
pub fn LimitsForm(
    /// "Most at once" text.
    max_in_flight_str: RwSignal<String>,
    /// Field message for "Most at once".
    max_in_flight_error: Signal<Option<String>>,
    /// "Enqueues per second" text.
    max_enqueue_str: RwSignal<String>,
    /// Field message for "Enqueues per second".
    max_enqueue_error: Signal<Option<String>>,
    /// Duplicate handling select value.
    idempotency: RwSignal<String>,
    /// Saved "most at once" limit (0 = none).
    saved_max_in_flight: Signal<u32>,
    /// Jobs of this task waiting or running now, once loaded.
    active: Signal<Option<u32>>,
) -> impl IntoView {
    let at_limit = Signal::derive(move || {
        active
            .get()
            .is_some_and(|a| boson_backend::at_in_flight_limit(a, saved_max_in_flight.get()))
    });

    view! {
        <Card>
            <BosonHelpCardHeader
                title="Limits"
                description="How much of this task type Boson accepts at once."
            />
            <Flex vertical=true gap=SpacingSize::Size160.flex_gap() padding=SpacingSize::Size160.inset()>
                {move || at_limit.get().then(|| {
                    let max = saved_max_in_flight.get();
                    view! {
                        <div data-testid="task-limits-at-limit">
                            <MessageBar intent=MessageBarIntent::Warning>
                                {format!(
                                    "At the limit: {max} of {max} jobs waiting or running. New enqueues are refused with Rate limited until one finishes; the apps that enqueue retry on their own."
                                )}
                            </MessageBar>
                        </div>
                    }
                })}
                <SettingsNumberField
                    label="Most at once"
                    testid="task-config-max-in-flight"
                    value=max_in_flight_str
                    error=max_in_flight_error
                    hint="Waiting and running together. 0 means no limit. Past it, enqueue is refused as Rate limited, not queued."
                />
                {move || active.get().map(|now| {
                    let max = saved_max_in_flight.get();
                    view! {
                        <Flex vertical=true gap=SpacingSize::Size40.flex_gap()>
                            {(max > 0).then(|| view! {
                                <ProgressBar
                                    value=Signal::stored(f64::from(now.min(max)))
                                    max=Signal::stored(f64::from(max))
                                />
                            })}
                            <div data-testid="task-limits-usage">
                                <Caption1>
                                    {if max > 0 {
                                        format!("{now} of {max} waiting or running now")
                                    } else {
                                        format!("{now} waiting or running now")
                                    }}
                                </Caption1>
                            </div>
                        </Flex>
                    }
                })}
                <SettingsNumberField
                    label="Enqueues per second"
                    testid="task-config-enqueue-rate"
                    value=max_enqueue_str
                    error=max_enqueue_error
                    hint="Per second, per server. 0 means no limit. Each server counts its own, so three servers allow up to 3 × this."
                />
                <Field label="Duplicate enqueues">
                    <div data-testid="task-config-idempotency">
                        <Select bind=idempotency>
                            <option value=EXACTLY_ONCE>"Exactly once (default)"</option>
                            <option value=AT_LEAST_ONCE>"At least once (faster)"</option>
                        </Select>
                    </div>
                    <FormHint>
                        "Only matters for jobs sent with an idempotency key. At least once skips the duplicate check, so a key can run twice."
                    </FormHint>
                </Field>
            </Flex>
        </Card>
    }
}
