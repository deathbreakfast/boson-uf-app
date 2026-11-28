mod basic_config_form;
mod form_state;
mod limits_form;
mod retry_policy_form;

pub use basic_config_form::TaskConfigForm;
pub use limits_form::LimitsForm;
pub use retry_policy_form::RetryPolicyForm;

use form_state::{use_task_config_form, TaskConfigFormState};

use leptos::prelude::*;
use leptos::server_fn::ServerFnError;
use orbital::components::{FormHint, Skeleton, SkeletonItem, SpacingSize};
use orbital::primitives::{
    Button, ButtonAppearance, Field, FieldValidationMessage, FieldValidationState, Flex, Input,
    InputAppearance, InputType, MessageBar, MessageBarIntent,
};

use crate::server::{
    get_task_active_jobs, get_task_config, list_gluon_pools_for_boson_task_config,
};

/// Skeleton placeholder while the Settings tab loads.
#[component]
fn TaskConfigSkeleton() -> impl IntoView {
    view! {
        <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
            <Skeleton>
                <SkeletonItem width="100%".to_string() height="160px".to_string() />
            </Skeleton>
            <Skeleton>
                <SkeletonItem width="100%".to_string() height="160px".to_string() />
            </Skeleton>
            <Skeleton>
                <SkeletonItem width="100%".to_string() height="160px".to_string() />
            </Skeleton>
        </Flex>
    }
}

/// A Settings number field: label, input, hint, and the field message when the text doesn't
/// parse.
#[component]
fn SettingsNumberField(
    /// Field label.
    label: &'static str,
    /// `data-testid` on the input wrapper.
    testid: &'static str,
    /// Two-way field text.
    value: RwSignal<String>,
    /// Field message when the text doesn't parse.
    error: Signal<Option<String>>,
    /// Hint under the input.
    #[prop(into)]
    hint: String,
) -> impl IntoView {
    view! {
        <Field label=label>
            <div data-testid=testid>
                <Input
                    bind=value
                    appearance=InputAppearance {
                        input_type: Signal::from(InputType::Text),
                        ..Default::default()
                    }
                />
            </div>
            <FormHint>{hint}</FormHint>
            {move || error.get().map(|message| view! {
                <div data-testid=format!("{testid}-error")>
                    <FieldValidationMessage state=FieldValidationState::Error(message) />
                </div>
            })}
        </Field>
    }
}

/// Save error, Save and Discard changes.
#[component]
fn SettingsActions(
    /// Form state.
    form: TaskConfigFormState,
    /// Save handler.
    on_save: Callback<leptos::ev::MouseEvent>,
    /// Save is off while saving or while a field doesn't parse.
    save_disabled: Signal<bool>,
) -> impl IntoView {
    view! {
        {move || form.save_error.get().map(|e| view! {
            <MessageBar intent=MessageBarIntent::Error>{e}</MessageBar>
        })}
        <Flex gap=SpacingSize::Size120.flex_gap()>
            <div data-testid="task-config-save">
                <Button
                    appearance=ButtonAppearance::Primary
                    disabled=save_disabled
                    on_click=on_save
                >
                    {move || if form.save_pending.get() { "Saving…" } else { "Save" }}
                </Button>
            </div>
            <div data-testid="task-config-discard">
                <Button
                    appearance=ButtonAppearance::Subtle
                    on_click=Callback::new(move |_| form.discard())
                >
                    "Discard changes"
                </Button>
            </div>
        </Flex>
    }
}

/// Field message for a parse result.
fn field_error<T: Clone + Send + Sync + 'static>(
    parsed: Memo<Result<T, String>>,
) -> Signal<Option<String>> {
    Signal::derive(move || parsed.get().err())
}

/// The queue list didn't load: say so; the Select offers the default queues meanwhile.
#[component]
fn PoolsLoadWarning(error: crate::errors::FriendlyError) -> impl IntoView {
    view! {
        <div data-testid="task-config-pools-error">
            <MessageBar intent=MessageBarIntent::Warning>
                {format!(
                    "Couldn't load this host's queues. {} The list shows the default queues until it loads.",
                    error.detail_for_list()
                )}
            </MessageBar>
        </div>
    }
}

/// The task type page's Settings tab: where it runs, retries and limits, with Save and
/// Discard changes. Requires verified email (the tab wraps it in `RequireAuthenticated`).
#[component]
pub fn BosonTaskConfigPage(
    /// Registry name of the task type.
    #[prop(into)]
    task_name: Signal<String>,
    /// Priority the task's code registers (shown in the priority hint).
    default_priority: i32,
) -> impl IntoView {
    let config_res = Resource::new(
        move || task_name.get(),
        move |name| async move {
            if name.is_empty() {
                return Err(ServerFnError::new("Missing task name"));
            }
            get_task_config(name).await
        },
    );
    let pools_res = Resource::new(|| (), |()| list_gluon_pools_for_boson_task_config());
    let active_res = Resource::new(
        move || task_name.get(),
        move |name| async move {
            if name.is_empty() {
                return Err(ServerFnError::new("Missing task name"));
            }
            get_task_active_jobs(name).await
        },
    );

    let form = use_task_config_form(config_res);
    let on_save = form.save_callback(task_name);
    let save_disabled = Signal::derive(move || form.save_pending.get() || !form.is_valid());
    let saved_max_in_flight = Signal::derive(move || {
        form.saved_config
            .get()
            .map_or(0, |c| c.rate_limit_policy.max_in_flight)
    });
    let active = Signal::derive(move || active_res.get().and_then(Result::ok));

    view! {
        <div data-testid="boson-task-config">
            <Suspense fallback=move || view! { <TaskConfigSkeleton /> }>
                {move || match (config_res.get(), pools_res.get()) {
                    (Some(Ok(_)), pools) => {
                        let pools_error = pools
                            .as_ref()
                            .and_then(|r| r.as_ref().err())
                            .map(crate::errors::friendly_error);
                        let pool_options = pools
                            .and_then(Result::ok)
                            .unwrap_or_else(boson_backend::default_gluon_pool_rows);
                        view! {
                            <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                                {pools_error.map(|e| view! { <PoolsLoadWarning error=e /> })}
                                {move || form.saved.get().then(|| view! {
                                    <div data-testid="task-config-saved">
                                        <MessageBar intent=MessageBarIntent::Success>
                                            "Saved. New jobs use these settings; jobs already waiting keep theirs."
                                        </MessageBar>
                                    </div>
                                })}
                                <TaskConfigForm
                                    pool=form.pool
                                    priority_str=form.priority
                                    priority_error=field_error(form.priority_parsed)
                                    default_priority=default_priority
                                    pool_options=pool_options
                                />
                                <RetryPolicyForm
                                    max_attempts_str=form.max_attempts
                                    max_attempts_error=field_error(form.max_attempts_parsed)
                                    first_wait_str=form.first_wait_s
                                    first_wait_error=field_error(form.first_wait_parsed)
                                    longest_wait_str=form.longest_wait_s
                                    longest_wait_error=field_error(form.longest_wait_parsed)
                                    multiplier_str=form.multiplier
                                    multiplier_error=field_error(form.multiplier_parsed)
                                    policy=form.retry_policy
                                />
                                <LimitsForm
                                    max_in_flight_str=form.max_in_flight
                                    max_in_flight_error=field_error(form.max_in_flight_parsed)
                                    max_enqueue_str=form.max_enqueue_per_second
                                    max_enqueue_error=field_error(form.max_enqueue_parsed)
                                    idempotency=form.idempotency
                                    saved_max_in_flight=saved_max_in_flight
                                    active=active
                                />
                                <SettingsActions form=form on_save=on_save save_disabled=save_disabled />
                            </Flex>
                        }
                        .into_any()
                    }
                    (Some(Err(e)), _) => view! {
                        <crate::errors::FriendlyErrorBar
                            error=crate::errors::friendly_error(&e)
                            data_testid="task-config-load-error"
                        />
                    }.into_any(),
                    (None, _) => view! { <TaskConfigSkeleton /> }.into_any(),
                }}
            </Suspense>
        </div>
    }
}
