use leptos::prelude::*;
use orbital::components::{
    Body1Strong, Caption1, Card, EmptyState, Skeleton, SkeletonItem, SkeletonItemSize, SpacingSize,
};
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, Flex, FlexAlign, FlexJustify, FlexWrap, Link, List,
    ListItem, MessageBar, MessageBarIntent,
};

use crate::components::{BosonCardContent, BosonHelpCardHeader};
use crate::errors::friendly_error;
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
                            <Flex align=FlexAlign::Center gap=SpacingSize::Size80.flex_gap()>
                                <Caption1>{format!("Owner: {}", line.owner)}</Caption1>
                                {line.yours.then(|| view! {
                                    <Badge
                                        appearance=BadgeAppearance::Tint
                                        color=BadgeColor::Brand
                                        attr:data-testid="boson-yours"
                                    >
                                        "Yours"
                                    </Badge>
                                })}
                            </Flex>
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

/// The lines in one list, or split into "Your teams (…)" and "Other teams" when some are the
/// reader's. The server sends the reader's lines first, each part worst first.
#[component]
fn NeedsAttentionGroups(lines: Vec<NeedsAttentionLine>) -> impl IntoView {
    let rows = |lines: Vec<(usize, NeedsAttentionLine)>| {
        view! {
            <List>
                {lines
                    .into_iter()
                    .map(|(index, line)| view! { <NeedsAttentionRow line=line index=index /> })
                    .collect_view()}
            </List>
        }
    };
    let (mine, others): (Vec<_>, Vec<_>) =
        lines.into_iter().enumerate().partition(|(_, l)| l.yours);
    if mine.is_empty() {
        return rows(others).into_any();
    }
    let mut teams: Vec<String> = Vec::new();
    for (_, line) in &mine {
        if !teams.contains(&line.owner) {
            teams.push(line.owner.clone());
        }
    }
    let heading = format!("Your teams ({})", teams.join(", "));
    view! {
        <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
            <div data-testid="needs-attention-yours">
                <Body1Strong>{heading}</Body1Strong>
                {rows(mine)}
            </div>
            {(!others.is_empty()).then(|| view! {
                <div data-testid="needs-attention-others">
                    <Body1Strong>"Other teams"</Body1Strong>
                    {rows(others)}
                </div>
            })}
        </Flex>
    }
    .into_any()
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
                            Ok(lines) => view! { <NeedsAttentionGroups lines=lines /> }.into_any(),
                            Err(e) => view! {
                                <MessageBar intent=MessageBarIntent::Error>
                                    {format!("Couldn't check what needs attention. {}", friendly_error(&e).sentence())}
                                </MessageBar>
                            }.into_any(),
                        })}
                    </Transition>
                </BosonCardContent>
            </Card>
        </div>
    }
}
