//! Yours first, Only yours or Everyone's: the Show select and caption above every list.
//!
//! The choice lives in the URL as `?show=` (`mine`, `all`; missing means Yours first), so a
//! link keeps it and back and forward restore it.

use boson_backend::{show_caption, ShowMode};
use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use leptos_router::NavigateOptions;
use orbital::components::{Caption1, SpacingSize};
use orbital::primitives::{Flex, FlexAlign, FlexJustify, FlexWrap, Select};
use orbital_data::DataValue;
use orbital_paging::{FilterLogicWire, FilterQuery, FilterRuleParam, PageRequest};

use crate::server::get_list_ownership;

/// What a list is grouped by, for the caption and the ownership lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedKind {
    /// Lists of task types, or of jobs and runs (grouped by their task type).
    TaskTypes,
    /// The Queues list.
    Queues,
}

impl OwnedKind {
    const fn key(self) -> &'static str {
        match self {
            Self::TaskTypes => "task_types",
            Self::Queues => "queues",
        }
    }

    const fn noun(self) -> &'static str {
        match self {
            Self::TaskTypes => "task types",
            Self::Queues => "queues",
        }
    }
}

/// The page's `?show=` mode, and a callback that changes it in the URL (keeping every other
/// query value).
pub fn use_show_mode() -> (Memo<ShowMode>, Callback<ShowMode>) {
    let query = use_query_map();
    let location = use_location();
    let navigate = use_navigate();
    let mode =
        Memo::new(move |_| ShowMode::from_key(&query.with(|q| q.get("show").unwrap_or_default())));
    let set = Callback::new(move |next: ShowMode| {
        let mut params = query.get_untracked();
        params.remove("show");
        if next != ShowMode::YoursFirst {
            params.insert("show", next.key().to_string());
        }
        let path = location.pathname.get_untracked();
        let href = format!("{path}{}", params.to_query_string());
        navigate(
            &href,
            NavigateOptions {
                replace: true,
                scroll: false,
                ..NavigateOptions::default()
            },
        );
    });
    (mode, set)
}

/// The Show select: Yours first (the default), Only yours, Everyone's.
#[component]
pub fn ShowSelect(
    /// Current mode, from the URL.
    #[prop(into)]
    mode: Signal<ShowMode>,
    /// Called with the picked mode.
    on_change: Callback<ShowMode>,
) -> impl IntoView {
    let value = RwSignal::new(mode.get_untracked().key().to_string());
    Effect::new(move |_| {
        let key = mode.get().key().to_string();
        if value.get_untracked() != key {
            value.set(key);
        }
    });
    Effect::new(move |_| {
        let picked = ShowMode::from_key(&value.get());
        if picked != mode.get_untracked() {
            on_change.run(picked);
        }
    });
    view! {
        <div data-testid="boson-show-select">
            <Select bind=value>
                <option value="first">"Yours first"</option>
                <option value="mine">"Only yours"</option>
                <option value="all">"Everyone's"</option>
            </Select>
        </div>
    }
}

/// The line above a list ("Yours first: task types owned by Media platform, then everyone
/// else's.") with the Show select beside it.
///
/// The ownership loads in the browser after hydration, so the list never waits on it.
#[component]
pub fn ListHeader(
    /// What the list is grouped by.
    kind: OwnedKind,
    /// Current mode, from the URL.
    #[prop(into)]
    mode: Signal<ShowMode>,
    /// Called with the picked mode.
    on_change: Callback<ShowMode>,
) -> impl IntoView {
    let teams = RwSignal::new(None::<Vec<String>>);
    Effect::new(move |_| {
        leptos::task::spawn_local_scoped(async move {
            let owned = get_list_ownership(kind.key().to_string()).await;
            teams.set(Some(owned.map_or_else(|_| Vec::new(), |o| o.teams)));
        });
    });
    let caption = move || {
        teams
            .get()
            .map(|teams| show_caption(mode.get(), kind.noun(), &teams))
    };
    view! {
        <Flex
            justify=FlexJustify::SpaceBetween
            align=FlexAlign::Center
            wrap=FlexWrap::Wrap
            gap=SpacingSize::Size80.flex_gap()
        >
            <span data-testid="boson-list-caption">
                <Caption1>{caption}</Caption1>
            </span>
            <ShowSelect mode=mode on_change=on_change />
        </Flex>
    }
}

/// Add (or replace) the `show` rule on a page request.
pub fn add_show_rule(request: &mut PageRequest, mode: ShowMode) {
    let filter = request.filter.get_or_insert_with(|| FilterQuery {
        items: Vec::new(),
        logic: FilterLogicWire::And,
    });
    filter
        .items
        .retain(|r| r.field != boson_backend::SHOW_FIELD);
    filter.items.push(FilterRuleParam {
        field: boson_backend::SHOW_FIELD.to_string(),
        operator: "is".to_string(),
        value: DataValue::Text(mode.key().to_string()),
    });
}
