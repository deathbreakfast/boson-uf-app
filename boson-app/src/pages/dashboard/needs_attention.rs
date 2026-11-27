use leptos::prelude::*;
use orbital::components::{
    Body1Strong, Caption1, Card, EmptyState, Skeleton, SkeletonItem, SkeletonItemSize, SpacingSize,
};
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, Flex, FlexAlign, FlexJustify, FlexWrap, Link, List,
    ListItem, MessageBar, MessageBarIntent,
};

use crate::components::{BosonCardContent, BosonHelpCardHeader};
use crate::server::{AttentionKind, NeedsAttentionLine};

#[component]
fn NeedsAttentionSkeleton() -> impl IntoView {
    view! {
        <Skeleton>
            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                {(0..3)
                    .map(|_| view! { <SkeletonItem size=Signal::from(SkeletonItemSize::S16) /> })
                    .collect_view()}
            </Flex>
        </Skeleton>
    }
}

#[component]
fn NeedsAttentionRow(
    /// One line, already worded.
    line: NeedsAttentionLine,
    /// Position in the list, for test ids.
    index: usize,
) -> impl IntoView {
    let color = match line.kind {
        AttentionKind::Failed => BadgeColor::Danger,
        AttentionKind::Slow => BadgeColor::Warning,
    };
    view! {
        <ListItem>
            <div data-testid=format!("needs-attention-line-{index}")>
                <Flex
                    justify=FlexJustify::SpaceBetween
                    align=FlexAlign::Center
                    wrap=FlexWrap::Wrap
                    gap=SpacingSize::Size120.flex_gap()
                >
                    <Flex align=FlexAlign::Start gap=SpacingSize::Size120.flex_gap()>
                        <Badge appearance=BadgeAppearance::Tint color=color>
                            {line.kind.label()}
                        </Badge>
                        <Flex vertical=true gap=SpacingSize::Size40.flex_gap()>
                            <Body1Strong>{line.title}</Body1Strong>
                            <Caption1>{line.detail}</Caption1>
                            <Caption1>{format!("Owner: {}", line.owner)}</Caption1>
                        </Flex>
                    </Flex>
                    <Link href=line.href attr:data-testid=format!("needs-attention-link-{index}")>
                        {line.link_label}
                    </Link>
                </Flex>
            </div>
        </ListItem>
    }
}

/// Needs attention: what's stuck or failing, worst first, each with a link to the list behind
/// it. Says All clear when nothing applies.
#[component]
pub fn NeedsAttentionCard(
    /// Resource that loads the lines.
    lines_res: Resource<Result<Vec<NeedsAttentionLine>, ServerFnError>>,
) -> impl IntoView {
    view! {
        <div data-testid="dashboard-needs-attention">
            <Card>
                <BosonHelpCardHeader title="Needs attention" />
                <BosonCardContent>
                    <Transition fallback=move || view! { <NeedsAttentionSkeleton /> }>
                        {move || lines_res.get().map(|r| match r {
                            Ok(lines) if lines.is_empty() => view! {
                                <div data-testid="needs-attention-all-clear">
                                    <EmptyState
                                        message="All clear"
                                        description="No failed jobs, and every queue is keeping up."
                                        icon=icondata::AiCheckCircleOutlined
                                    />
                                </div>
                            }.into_any(),
                            Ok(lines) => view! {
                                <List>
                                    {lines
                                        .into_iter()
                                        .enumerate()
                                        .map(|(index, line)| view! { <NeedsAttentionRow line=line index=index /> })
                                        .collect_view()}
                                </List>
                            }.into_any(),
                            Err(e) => view! {
                                <MessageBar intent=MessageBarIntent::Error>
                                    "Couldn't check what needs attention. " {e.to_string()}
                                </MessageBar>
                            }.into_any(),
                        })}
                    </Transition>
                </BosonCardContent>
            </Card>
        </div>
    }
}
