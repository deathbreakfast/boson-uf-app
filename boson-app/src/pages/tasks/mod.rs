mod task_card;

pub use task_card::TaskCard;

use crate::components::{boson_table_page_layout, BosonCardContent, TasksDataTable};
use leptos::prelude::*;
use orbital::components::{Body1, Card, ContentContainer, SpacingSize, Title3};
use orbital::primitives::Flex;

/// Task index: searchable list of all configured tasks.
#[component]
pub fn BosonTasksIndexPage() -> impl IntoView {
    let (page_style, page_classes) = boson_table_page_layout();

    view! {
        <style>{page_style}</style>
        <ContentContainer class=page_classes.page data_testid="boson-tasks">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap() class=page_classes.body>
                <Flex vertical=true gap=SpacingSize::Size40.flex_gap()>
                    <Title3>"Tasks"</Title3>
                    <Body1>"Every task type registered with #[boson::task] on this host. Open one to see its runs and settings."</Body1>
                </Flex>

                <Card class=page_classes.card>
                    <BosonCardContent class=page_classes.card_content>
                        <div data-testid="tasks-search">
                            <TasksDataTable />
                        </div>
                    </BosonCardContent>
                </Card>
            </Flex>
        </ContentContainer>
    }
}
