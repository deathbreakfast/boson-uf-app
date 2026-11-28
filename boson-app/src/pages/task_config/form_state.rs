use leptos::prelude::*;
use leptos::server_fn::ServerFnError;
use leptos::task::spawn_local_scoped;

use boson_backend::{
    ms_to_seconds_field, parse_multiplier_field, parse_wait_seconds_field,
    parse_whole_number_field, MAX_RETRY_ATTEMPTS, MAX_TASK_PRIORITY, MIN_TASK_PRIORITY,
};

use crate::server::{
    update_task_config, IdempotencyModeDto, RateLimitPolicyDto, RetryPolicyDto, TaskConfigDto,
    UpdateTaskConfigRequest,
};

/// Select value for [`IdempotencyModeDto::ExactlyOnce`].
pub const EXACTLY_ONCE: &str = "exactly_once";
/// Select value for [`IdempotencyModeDto::AtLeastOnce`].
pub const AT_LEAST_ONCE: &str = "at_least_once";

/// Largest limit the form accepts for "Most at once" and "Enqueues per second".
const MAX_LIMIT: i64 = u32::MAX as i64;

/// Field message when a limit isn't a whole number.
const LIMIT_MESSAGE: &str = "Enter a whole number, 0 or more. 0 means no limit.";

/// Form field text, one parse result per field, and the save handler for the Settings tab.
///
/// Each field keeps the text the person typed. A field that doesn't parse shows its message
/// and turns Save off; nothing falls back to the old value.
#[derive(Clone, Copy)]
pub struct TaskConfigFormState {
    pub pool: RwSignal<String>,
    pub priority: RwSignal<String>,
    pub max_attempts: RwSignal<String>,
    pub first_wait_s: RwSignal<String>,
    pub longest_wait_s: RwSignal<String>,
    pub multiplier: RwSignal<String>,
    pub max_in_flight: RwSignal<String>,
    pub max_enqueue_per_second: RwSignal<String>,
    pub idempotency: RwSignal<String>,
    pub priority_parsed: Memo<Result<i32, String>>,
    pub max_attempts_parsed: Memo<Result<u32, String>>,
    pub first_wait_parsed: Memo<Result<u64, String>>,
    pub longest_wait_parsed: Memo<Result<u64, String>>,
    pub multiplier_parsed: Memo<Result<f64, String>>,
    pub max_in_flight_parsed: Memo<Result<u32, String>>,
    pub max_enqueue_parsed: Memo<Result<u32, String>>,
    /// The retry policy when every retry field parses.
    pub retry_policy: Memo<Option<RetryPolicyDto>>,
    /// The config as loaded or last saved (Discard changes goes back to it).
    pub saved_config: RwSignal<Option<TaskConfigDto>>,
    pub save_pending: RwSignal<bool>,
    pub save_error: RwSignal<Option<String>>,
    pub saved: RwSignal<bool>,
}

fn parse_u32_limit(raw: &str) -> Result<u32, String> {
    parse_whole_number_field(raw, 0, MAX_LIMIT)
        .ok()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| LIMIT_MESSAGE.to_string())
}

impl TaskConfigFormState {
    pub fn new() -> Self {
        let text = || RwSignal::new(String::new());
        let pool = RwSignal::new("global".to_string());
        let priority = text();
        let max_attempts = text();
        let first_wait_s = text();
        let longest_wait_s = text();
        let multiplier = text();
        let max_in_flight = text();
        let max_enqueue_per_second = text();
        let idempotency = RwSignal::new(EXACTLY_ONCE.to_string());

        let priority_parsed = Memo::new(move |_| {
            parse_whole_number_field(
                &priority.get(),
                i64::from(MIN_TASK_PRIORITY),
                i64::from(MAX_TASK_PRIORITY),
            )
            .and_then(|n| i32::try_from(n).map_err(|e| e.to_string()))
        });
        let max_attempts_parsed = Memo::new(move |_| {
            parse_whole_number_field(&max_attempts.get(), 1, i64::from(MAX_RETRY_ATTEMPTS))
                .and_then(|n| u32::try_from(n).map_err(|e| e.to_string()))
        });
        let first_wait_parsed = Memo::new(move |_| parse_wait_seconds_field(&first_wait_s.get()));
        let longest_wait_parsed = Memo::new(move |_| {
            let longest = parse_wait_seconds_field(&longest_wait_s.get())?;
            match first_wait_parsed.get() {
                Ok(first) if longest < first => {
                    Err("The longest wait can't be shorter than the first wait.".to_string())
                }
                _ => Ok(longest),
            }
        });
        let multiplier_parsed = Memo::new(move |_| parse_multiplier_field(&multiplier.get()));
        let max_in_flight_parsed = Memo::new(move |_| parse_u32_limit(&max_in_flight.get()));
        let max_enqueue_parsed = Memo::new(move |_| parse_u32_limit(&max_enqueue_per_second.get()));

        let retry_policy = Memo::new(move |_| {
            Some(RetryPolicyDto {
                max_attempts: max_attempts_parsed.get().ok()?,
                base_delay_ms: first_wait_parsed.get().ok()?,
                backoff_multiplier: multiplier_parsed.get().ok()?,
                max_delay_ms: longest_wait_parsed.get().ok()?,
            })
        });

        Self {
            pool,
            priority,
            max_attempts,
            first_wait_s,
            longest_wait_s,
            multiplier,
            max_in_flight,
            max_enqueue_per_second,
            idempotency,
            priority_parsed,
            max_attempts_parsed,
            first_wait_parsed,
            longest_wait_parsed,
            multiplier_parsed,
            max_in_flight_parsed,
            max_enqueue_parsed,
            retry_policy,
            saved_config: RwSignal::new(None),
            save_pending: RwSignal::new(false),
            save_error: RwSignal::new(None),
            saved: RwSignal::new(false),
        }
    }

    /// Fill every field from a loaded or saved config.
    pub fn populate(&self, c: &TaskConfigDto) {
        self.pool.set(c.pool.clone());
        self.priority.set(c.priority.to_string());
        self.max_attempts
            .set(c.retry_policy.max_attempts.to_string());
        self.first_wait_s
            .set(ms_to_seconds_field(c.retry_policy.base_delay_ms));
        self.longest_wait_s
            .set(ms_to_seconds_field(c.retry_policy.max_delay_ms));
        self.multiplier
            .set(c.retry_policy.backoff_multiplier.to_string());
        self.max_in_flight
            .set(c.rate_limit_policy.max_in_flight.to_string());
        self.max_enqueue_per_second
            .set(c.rate_limit_policy.max_enqueue_per_second.to_string());
        self.idempotency.set(
            match c.idempotency_mode {
                IdempotencyModeDto::ExactlyOnce => EXACTLY_ONCE,
                IdempotencyModeDto::AtLeastOnce => AT_LEAST_ONCE,
            }
            .to_string(),
        );
        self.saved_config.set(Some(c.clone()));
    }

    pub fn wire_populate_effect(&self, config_res: Resource<Result<TaskConfigDto, ServerFnError>>) {
        let form = *self;
        Effect::new(move |_| {
            if let Some(Ok(c)) = config_res.get() {
                form.populate(&c);
            }
        });
    }

    /// Every field parses, so Save can run.
    pub fn is_valid(&self) -> bool {
        self.priority_parsed.with(Result::is_ok)
            && self.retry_policy.with(Option::is_some)
            && self.max_in_flight_parsed.with(Result::is_ok)
            && self.max_enqueue_parsed.with(Result::is_ok)
    }

    /// Put every field back to the loaded or last saved config.
    pub fn discard(&self) {
        if let Some(c) = self.saved_config.get_untracked() {
            self.populate(&c);
        }
        self.save_error.set(None);
        self.saved.set(false);
    }

    fn request(&self) -> Option<UpdateTaskConfigRequest> {
        Some(UpdateTaskConfigRequest {
            priority: Some(self.priority_parsed.get_untracked().ok()?),
            pool: Some(self.pool.get_untracked()),
            retry_policy: Some(self.retry_policy.get_untracked()?),
            rate_limit_policy: Some(RateLimitPolicyDto {
                max_in_flight: self.max_in_flight_parsed.get_untracked().ok()?,
                max_enqueue_per_second: self.max_enqueue_parsed.get_untracked().ok()?,
            }),
            idempotency_mode: Some(if self.idempotency.get_untracked() == AT_LEAST_ONCE {
                IdempotencyModeDto::AtLeastOnce
            } else {
                IdempotencyModeDto::ExactlyOnce
            }),
        })
    }

    pub fn save_callback(&self, task_name: Signal<String>) -> Callback<leptos::ev::MouseEvent> {
        let form = *self;
        Callback::new(move |_| {
            let name = task_name.get_untracked();
            if name.is_empty() {
                return;
            }
            let Some(req) = form.request() else {
                return;
            };
            form.save_pending.set(true);
            form.save_error.set(None);
            form.saved.set(false);
            spawn_local_scoped(async move {
                let result = update_task_config(name, req).await;
                form.save_pending.set(false);
                match result {
                    Ok(c) => {
                        form.populate(&c);
                        form.saved.set(true);
                    }
                    Err(e) => form
                        .save_error
                        .set(Some(crate::errors::friendly_error(&e).sentence())),
                }
            });
        })
    }
}

pub fn use_task_config_form(
    config_res: Resource<Result<TaskConfigDto, ServerFnError>>,
) -> TaskConfigFormState {
    let form = TaskConfigFormState::new();
    form.wire_populate_effect(config_res);
    form
}
