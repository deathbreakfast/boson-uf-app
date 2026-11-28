//! A list's failed load: "Couldn't load runs. Boson's coordinator didn't answer. Your filters
//! are kept." with Try again, in place of an empty table.
//!
//! Each table's fetcher goes through [`track_load_error`], which keeps the last failure in a
//! signal and clears it once a page loads. The table's empty and no-results slots show
//! [`TableLoadError`] while it is set, so the toolbar, search and filters stay. Try again bumps
//! the table's refresh signal, which sends the same request again.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use leptos::prelude::*;
use orbital::components::{MessageBar, MessageBarIntent, SpacingSize};
use orbital::primitives::{Button, ButtonAppearance, Flex, FlexAlign, FlexWrap, PageFetcher};
use orbital_data::DataRecord;
use orbital_paging::{Page, PageRequest};

use crate::errors::{friendly_error, FriendlyError};

/// The last failed load of one table, `None` once a page loads.
pub type LoadError = RwSignal<Option<FriendlyError>>;

/// Wrap a fetcher so its failures land in `error` (in words) and a good page clears it.
pub fn track_load_error(fetcher: PageFetcher, error: LoadError) -> PageFetcher {
    Arc::new(move |request: PageRequest| {
        let fut = fetcher(request);
        Box::pin(async move {
            let result = fut.await;
            match &result {
                Ok(_) => {
                    if error.get_untracked().is_some() {
                        error.set(None);
                    }
                }
                Err(e) => error.set(Some(friendly_error(e))),
            }
            result
        }) as Pin<Box<dyn Future<Output = Result<Page<DataRecord>, ServerFnError>> + Send>>
    })
}

/// The failed-load bar with Try again, or `fallback` when the last load worked.
#[component]
pub fn TableLoadError(
    /// The table's last failed load.
    error: LoadError,
    /// What the table lists, for "Couldn't load runs."
    noun: &'static str,
    /// Sends the same request again (bump the table's refresh signal).
    on_retry: Callback<()>,
    /// What the slot shows when the load worked (the empty or no-match state).
    children: ChildrenFn,
) -> impl IntoView {
    move || {
        let Some(e) = error.get() else {
            return children().into_any();
        };
        view! {
            <div data-testid="boson-table-load-error">
                <MessageBar intent=MessageBarIntent::Error>
                    <Flex align=FlexAlign::Center wrap=FlexWrap::Wrap gap=SpacingSize::Size120.flex_gap()>
                        <span>
                            {format!("Couldn't load {noun}. {} Your filters are kept.", e.detail_for_list())}
                        </span>
                        <span data-testid="boson-table-load-error-retry">
                            <Button
                                appearance=ButtonAppearance::Secondary
                                on_click=Callback::new(move |_| on_retry.run(()))
                            >
                                "Try again"
                            </Button>
                        </span>
                    </Flex>
                </MessageBar>
            </div>
        }
        .into_any()
    }
}
