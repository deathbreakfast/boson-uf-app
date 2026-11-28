//! Plain-language errors for every message bar that reports a failed server call.
//!
//! [`friendly_error`] turns a [`ServerFnError`] into a title and a detail sentence that say
//! what happened and what to do next. Permission denials name the access that's missing,
//! validation errors name the field and its range, and transport or coordinator failures say
//! Boson didn't answer. The server function's own text never reaches the screen.

use leptos::prelude::*;
use orbital::components::{MessageBar, MessageBarIntent};
use orbital::primitives::{Body1, Flex};

use crate::permissions::{access_label, denied_resource, BOSON_QUEUE_KIND};

/// A failed call in words: a short title and one or two sentences of detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendlyError {
    /// What happened, as a sentence without a full stop ("Boson's coordinator didn't answer").
    pub title: String,
    /// Why, and what to do next.
    pub detail: String,
}

impl FriendlyError {
    fn new(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
        }
    }

    /// The cause for a list's failed-load line: "Boson's coordinator didn't answer." for a
    /// transport failure, else the title as a sentence.
    #[must_use]
    pub fn detail_for_list(&self) -> String {
        format!("{}.", self.title)
    }

    /// Title and detail as one line: "Title. Detail".
    #[must_use]
    pub fn sentence(&self) -> String {
        format!("{}. {}", self.title, self.detail)
    }
}

/// [`friendly_error`] without owners to name in a permission denial.
#[must_use]
pub fn friendly_error(error: &ServerFnError) -> FriendlyError {
    friendly_error_with_owners(error, None)
}

/// A server error in words. `owners` names who owns the resource, for a permission denial
/// ("That needs Can edit on `orders.sync_invoices`. Owners: Boson admins.").
#[must_use]
pub fn friendly_error_with_owners(error: &ServerFnError, owners: Option<&str>) -> FriendlyError {
    friendly_message(&error.to_string(), owners)
}

/// [`friendly_error_with_owners`] for the error's text.
#[must_use]
pub fn friendly_message(message: &str, owners: Option<&str>) -> FriendlyError {
    if let Some((resource, action)) = denied_resource(message) {
        let queue = resource.kind == BOSON_QUEUE_KIND;
        let title = match (queue, action.as_str()) {
            (false, "Edit") => "You can't change this task type's settings",
            (false, "Run") => "You can't act on this task type's jobs",
            (false, _) => "You can't see this task type",
            (true, "Edit") => "You can't pause or resume this queue",
            (true, "Delete") => "You can't purge this queue",
            (true, _) => "You can't see this queue",
        };
        let role = match action.as_str() {
            "Delete" => "Owner",
            other => access_label(&[other]),
        };
        let owners = owners.map_or_else(
            || " Ask one of its owners for access.".to_string(),
            |o| format!(" Owners: {o}."),
        );
        return FriendlyError::new(
            title,
            format!("That needs {role} on {}.{owners}", resource.id),
        );
    }
    if let Some(rest) = after(message, uf_product_access_denied_prefix()) {
        // uf_product writes "You need <action> on <resource>."; anything else stays generic.
        let rest = rest.trim();
        let detail =
            if rest.starts_with("You need ") && rest.contains(" on ") && rest.ends_with('.') {
                rest.to_string()
            } else {
                "Ask one of its owners for access.".to_string()
            };
        return FriendlyError::new("You don't have access to this", detail);
    }
    if let Some(permission) = after(message, "permission_denied::") {
        let permission = permission.split_whitespace().next().unwrap_or_default();
        return FriendlyError::new(
            "You don't have access to this",
            format!("That needs the {permission} permission. Ask a Boson admin."),
        );
    }
    if let Some(field) = after(message, "Invalid task config update:") {
        return FriendlyError::new("That value is out of range", validation_sentence(field));
    }
    let lower = message.to_lowercase();
    if lower.contains("failed to cancel job") {
        return if lower.contains("not found") {
            FriendlyError::new(
                "That job no longer exists",
                "It finished or was removed. The list is up to date now.",
            )
        } else {
            FriendlyError::new(
                "That job already finished, so there's nothing to cancel",
                "The list is up to date now.",
            )
        };
    }
    if lower.contains("backend not in request context") || lower.contains("embedded boson runtime")
    {
        return FriendlyError::new(
            "Boson isn't running on this host",
            "The site's background worker isn't wired in. Ask whoever runs this deployment.",
        );
    }
    if lower.contains("authentication is required") || lower.contains("not signed in") {
        return FriendlyError::new("You're signed out", "Sign in again to keep going.");
    }
    if lower.contains("not found") {
        return FriendlyError::new(
            "That's no longer here",
            "It may have been removed. Go back and pick it again.",
        );
    }
    // Only Boson's own job-selection messages pass through; their wording is fixed.
    if lower.contains("choose at least one job") {
        return FriendlyError::new("Pick a job first", "Choose at least one job.");
    }
    if lower.contains("choose at most") {
        return FriendlyError::new(
            "Pick fewer jobs",
            format!(
                "Choose at most {} jobs at once.",
                boson_backend::MAX_PAGE_LIST_LIMIT
            ),
        );
    }
    FriendlyError::new(
        "Boson's coordinator didn't answer",
        "Nothing changed. Try again in a moment.",
    )
}

const fn uf_product_access_denied_prefix() -> &'static str {
    "ACCESS:denied:"
}

/// The text after `prefix`, wherever it appears in `message`.
fn after<'a>(message: &'a str, prefix: &str) -> Option<&'a str> {
    message.find(prefix).map(|i| &message[i + prefix.len()..])
}

/// "priority must be between -10 and 10" → "Priority must be between -10 and 10."
fn validation_sentence(raw: &str) -> String {
    let text = raw
        .trim()
        .replace("max_attempts", "attempts")
        .replace("backoff_multiplier", "the backoff multiplier");
    sentence_case(&text)
}

fn sentence_case(raw: &str) -> String {
    let text = raw.trim().trim_end_matches('.');
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        format!("{}{}.", first.to_uppercase(), chars.as_str())
    })
}

/// An error message bar: the title in bold, then the detail.
#[component]
pub fn FriendlyErrorBar(
    /// The error to show.
    error: FriendlyError,
    /// Test id on the wrapper.
    #[prop(optional, into)]
    data_testid: Option<String>,
) -> impl IntoView {
    view! {
        <div data-testid=data_testid.unwrap_or_else(|| "boson-error".to_string())>
            <MessageBar intent=MessageBarIntent::Error>
                <Flex vertical=true>
                    <Body1><strong>{error.title}</strong></Body1>
                    <Body1>{error.detail}</Body1>
                </Flex>
            </MessageBar>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(text: &str) -> FriendlyError {
        friendly_message(text, None)
    }

    #[test]
    fn permission_denied_names_the_role_and_owners() {
        let e = friendly_message(
            "error running server function: permission_denied::boson_task_type.orders.sync_invoices.Edit",
            Some("Boson admins"),
        );
        assert_eq!(e.title, "You can't change this task type's settings");
        assert_eq!(
            e.detail,
            "That needs Can edit on orders.sync_invoices. Owners: Boson admins."
        );
        let q = msg("permission_denied::boson_queue.billing.Delete");
        assert_eq!(q.title, "You can't purge this queue");
        assert!(q.detail.starts_with("That needs Owner on billing."));
    }

    #[test]
    fn app_permission_denied_names_the_permission() {
        let e = msg("error running server function: permission_denied::BosonAdmin");
        assert_eq!(
            e.detail,
            "That needs the BosonAdmin permission. Ask a Boson admin."
        );
    }

    #[test]
    fn validation_names_the_field_and_range() {
        let e = msg("error running server function: Invalid task config update: priority must be between -10 and 10");
        assert_eq!(e.title, "That value is out of range");
        assert_eq!(e.detail, "Priority must be between -10 and 10.");
    }

    #[test]
    fn cancel_and_backend_errors_read_as_sentences() {
        assert_eq!(
            msg("Failed to cancel job: invalid state").title,
            "That job already finished, so there's nothing to cancel"
        );
        assert_eq!(
            msg("Boson backend not in request context").title,
            "Boson isn't running on this host"
        );
        assert_eq!(
            msg("error running server function: Failed to load queue stats: timeout").title,
            "Boson's coordinator didn't answer"
        );
    }

    #[test]
    fn unknown_backend_text_never_passes_through() {
        for raw in [
            "error running server function: invalid column \"secret_token\" in table jobs",
            "ACCESS:denied: backend said: db row 42 locked",
            "this value must be shorter than internal_limit_x",
        ] {
            let shown = msg(raw).sentence();
            assert!(!shown.contains("secret_token"), "{shown}");
            assert!(!shown.contains("row 42"), "{shown}");
            assert!(!shown.contains("internal_limit_x"), "{shown}");
        }
        assert_eq!(
            msg("ACCESS:denied: You need Edit on billing.").detail,
            "You need Edit on billing."
        );
    }

    #[test]
    fn no_server_function_text_reaches_the_screen() {
        for raw in [
            "error running server function: anything at all",
            "error reaching server to call server function: TypeError",
            "Failed to count jobs: db locked",
            "permission_denied::boson_task_type.e2e_echo.Run",
        ] {
            let e = msg(raw);
            let shown = e.sentence();
            assert!(!shown.contains("server function"), "{shown}");
            assert!(!shown.contains("permission_denied"), "{shown}");
        }
    }
}
