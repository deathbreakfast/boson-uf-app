use leptos::prelude::*;
use orbital::components::{Caption1, Card, SpacingSize};
use orbital::primitives::Flex;

use crate::components::BosonHelpCardHeader;
use crate::server::RetryPolicyDto;

use super::SettingsNumberField;

/// "Retries" card: total tries, waits in seconds, growth, and what those values do.
#[component]
pub fn RetryPolicyForm(
    /// Max attempts text.
    max_attempts_str: RwSignal<String>,
    /// Field message for max attempts.
    max_attempts_error: Signal<Option<String>>,
    /// First wait text, in seconds.
    first_wait_str: RwSignal<String>,
    /// Field message for the first wait.
    first_wait_error: Signal<Option<String>>,
    /// Longest wait text, in seconds.
    longest_wait_str: RwSignal<String>,
    /// Field message for the longest wait.
    longest_wait_error: Signal<Option<String>>,
    /// Multiplier text.
    multiplier_str: RwSignal<String>,
    /// Field message for the multiplier.
    multiplier_error: Signal<Option<String>>,
    /// The policy when every field parses.
    policy: Memo<Option<RetryPolicyDto>>,
) -> impl IntoView {
    view! {
        <Card>
            <BosonHelpCardHeader
                title="Retries"
                description="What Boson does when a run fails."
            />
            <Flex vertical=true gap=SpacingSize::Size160.flex_gap() padding=SpacingSize::Size160.inset()>
                <SettingsNumberField
                    label="Max attempts"
                    testid="task-config-max-attempts"
                    value=max_attempts_str
                    error=max_attempts_error
                    hint="Total tries, including the first. 1 turns retries off. 1 to 20."
                />
                <SettingsNumberField
                    label="First wait (seconds)"
                    testid="task-config-first-wait"
                    value=first_wait_str
                    error=first_wait_error
                    hint="How long to wait before the first retry."
                />
                <SettingsNumberField
                    label="Longest wait (seconds)"
                    testid="task-config-longest-wait"
                    value=longest_wait_str
                    error=longest_wait_error
                    hint="The wait between retries never grows past this."
                />
                <SettingsNumberField
                    label="Wait grows by"
                    testid="task-config-multiplier"
                    value=multiplier_str
                    error=multiplier_error
                    hint="Each wait is the last one times this. 2 doubles it."
                />
                {move || policy.get().map(|p| view! {
                    <div data-testid="task-config-retry-sentence">
                        <Caption1>{boson_backend::retry_schedule_sentence(&p)}</Caption1>
                    </div>
                })}
            </Flex>
        </Card>
    }
}
