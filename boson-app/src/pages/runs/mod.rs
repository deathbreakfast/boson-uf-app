use crate::components::{
    boson_table_page_layout, BosonCardContent, RunsDataTable, RunsFilters, RunsTableScope,
};
use crate::server::list_task_names;
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_router::NavigateOptions;
use orbital::components::{Caption1, Card, ContentContainer, SpacingSize, Tag, Title3};
use orbital::primitives::{Button, ButtonAppearance, ButtonSize, Flex, FlexAlign};

/// Run history index: paginated list of past runs across all tasks.
///
/// Status, task and time filters live in the URL (`?status=&task=&from=&to=`), so the dashboard
/// and other pages can link straight to a filtered list. Changing one reloads the rows in place.
#[component]
pub fn BosonRunsIndexPage() -> impl IntoView {
    let query = use_query_map();
    let navigate = use_navigate();
    let navigate_store = StoredValue::new(navigate);

    let job_filter = Memo::new(move |_| query.with(|q| q.get("job").filter(|s| !s.is_empty())));

    let scope = Memo::new(move |_| {
        job_filter
            .get()
            .map_or(RunsTableScope::All, RunsTableScope::ForJob)
    });

    let filters = Memo::new(move |_| {
        let scoped = job_filter.get().is_some();
        query.with(|q| {
            RunsFilters::from_query(
                q.get("status"),
                q.get("task"),
                q.get("from"),
                q.get("to"),
                scoped,
            )
        })
    });

    let on_filters_change = Callback::new(move |next: RunsFilters| {
        let job = job_filter.get_untracked();
        let query = next.to_query(job.as_deref());
        let href = if query.is_empty() {
            crate::paths::RUNS.to_string()
        } else {
            format!("{}?{query}", crate::paths::RUNS)
        };
        navigate_store.with_value(|nav| {
            nav(
                &href,
                NavigateOptions {
                    replace: true,
                    scroll: false,
                    ..NavigateOptions::default()
                },
            );
        });
    });

    let task_names = LocalResource::new(|| async { list_task_names().await.unwrap_or_default() });
    let task_names = Signal::derive(move || task_names.get().unwrap_or_default());

    let empty_description = Memo::new(move |_| {
        if job_filter.get().is_some() {
            "No runs found for the selected job."
        } else {
            "Run history will appear here when jobs execute."
        }
    });

    let (page_style, page_classes) = boson_table_page_layout();
    let fill_card_content_store = StoredValue::new(page_classes.card_content.clone());
    let fill_card_store = StoredValue::new(page_classes.card.clone());

    view! {
        <style>{page_style}</style>
        <ContentContainer class=page_classes.page data_testid="boson-runs">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap() class=page_classes.body>
                <Title3>"Runs"</Title3>

                {move || job_filter.get().map(|job_id| {
                    view! {
                        <div data-testid="runs-job-filter-chip">
                            <Flex align=FlexAlign::Center gap=SpacingSize::Size80.flex_gap()>
                                <Caption1>"Filtered by job:"</Caption1>
                                <Tag>{job_id}</Tag>
                                <Button
                                    size=ButtonSize::Small
                                    appearance=ButtonAppearance::Subtle
                                    on_click=Callback::new({
                                        let nav = navigate_store.with_value(Clone::clone);
                                        move |_| {
                                            nav(crate::paths::RUNS, NavigateOptions::default());
                                        }
                                    })
                                >
                                    "Clear"
                                </Button>
                            </Flex>
                        </div>
                    }
                })}

                // Rebuilt only when the job scope changes; filter changes reload rows in place.
                {move || {
                    let current_scope = scope.get();
                    let desc = empty_description.get();
                    view! {
                        <Card class=fill_card_store.with_value(Clone::clone)>
                            <BosonCardContent class=fill_card_content_store.with_value(Clone::clone)>
                                <RunsDataTable
                                    scope=current_scope
                                    filters=filters
                                    on_filters_change=on_filters_change
                                    task_names=task_names
                                    fill_height=true
                                    empty_description=desc
                                />
                            </BosonCardContent>
                        </Card>
                    }
                }}
            </Flex>
        </ContentContainer>
    }
}
