//! Access pieces shared by the Boson pages: the "Your access" badge and the denied state for
//! a server error that names a missing action.

use leptos::prelude::*;
use orbital::primitives::{Badge, BadgeAppearance, BadgeColor, MessageBar, MessageBarIntent};
use uf_product::access::AccessDenied;

use crate::permissions::{denied_resource, NO_ACCESS};

/// Badge for the reader's role on one task type or queue ("Owner", "Can run", "No access").
#[component]
pub fn AccessBadge(
    /// Role label from the list server functions.
    #[prop(into)]
    access: String,
) -> impl IntoView {
    let (appearance, color) = if access == NO_ACCESS {
        (BadgeAppearance::Outline, BadgeColor::Subtle)
    } else {
        (BadgeAppearance::Tint, BadgeColor::Informative)
    };
    view! {
        <Badge appearance=appearance color=color attr:data-testid="boson-your-access">{access}</Badge>
    }
}

/// [`AccessDenied`] when the error is a missing action on a task type or queue, otherwise the
/// error in a `MessageBar`.
///
/// `purpose` says what the action would let the reader do here ("see this queue's jobs").
pub fn server_fn_error_or_denied(error: &ServerFnError, purpose: &str) -> AnyView {
    if let Some((resource, action)) = denied_resource(&error.to_string()) {
        view! {
            <AccessDenied resource=resource action=action purpose=purpose.to_string() />
        }
        .into_any()
    } else {
        let message = error.to_string();
        view! { <MessageBar intent=MessageBarIntent::Error>{message}</MessageBar> }.into_any()
    }
}
