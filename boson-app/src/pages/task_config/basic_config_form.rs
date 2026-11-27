use leptos::prelude::*;
use orbital::components::{Card, FormHint, SpacingSize};
use orbital::primitives::{Field, Flex, Select};

use crate::components::BosonHelpCardHeader;
use crate::server::GluonPoolPickRow;

use super::SettingsNumberField;

/// "Where it runs" card: queue (pool) and priority.
#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn TaskConfigForm(
    /// Two-way signal holding the queue (pool) id.
    pool: RwSignal<String>,
    /// Two-way signal holding the priority text.
    priority_str: RwSignal<String>,
    /// Field message when the priority doesn't parse.
    priority_error: Signal<Option<String>>,
    /// Priority the task's code registers.
    default_priority: i32,
    /// Queues the host offers.
    #[prop(default = Vec::new())]
    pool_options: Vec<GluonPoolPickRow>,
) -> impl IntoView {
    let opts_hint = pool_options.clone();
    let priority_hint =
        format!("Lower runs sooner. -1000 to 10000. The code's default is {default_priority}.");

    view! {
        <Card>
            <BosonHelpCardHeader
                title="Where it runs"
                description="The queue that runs this task type and its place in that queue."
            />
            <Flex vertical=true gap=SpacingSize::Size160.flex_gap() padding=SpacingSize::Size160.inset()>
                <Field label="Queue">
                    <div data-testid="task-config-pool">
                        <Select bind=pool>
                            {pool_options.into_iter().map(|o| {
                                view! { <option value=o.id>{o.label}</option> }
                            }).collect_view()}
                        </Select>
                    </div>
                    <FormHint>{move || {
                        let p = pool.get();
                        opts_hint
                            .iter()
                            .find(|o| o.id == p)
                            .map_or_else(String::new, |o| o.detail.clone())
                    }}</FormHint>
                </Field>
                <SettingsNumberField
                    label="Priority"
                    testid="task-config-priority"
                    value=priority_str
                    error=priority_error
                    hint=priority_hint
                />
            </Flex>
        </Card>
    }
}
