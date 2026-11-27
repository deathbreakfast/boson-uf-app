use leptos::prelude::*;
use orbital::components::{Card, Subtitle2, Text, TextFont};
use orbital::primitives::{
    ButtonAppearance, CardHeader, CardHeaderAction, CopyButton, Flex, MessageBar, MessageBarIntent,
};

use crate::components::BosonCardContent;

/// What the error text is and who can see it.
const ERROR_DATA_NOTE: &str = "The handler wrote this text. It can hold URLs, account details or personal data, so only people who can view this task type see it. Boson keeps it 30 days.";

/// A failed run's error: a note on what it may contain, the text in monospace, and Copy.
#[component]
pub fn RunErrorDisplay(
    /// Message text to display.
    message: String,
) -> impl IntoView {
    let copy_text = Signal::stored(message.clone());
    let action = CardHeaderAction {
        children: Box::new(move || {
            view! {
                <CopyButton text=copy_text appearance=ButtonAppearance::Subtle />
            }
            .into_any()
        }),
    };

    view! {
        <Card>
            <CardHeader card_header_action=action>
                <Subtitle2>"Error"</Subtitle2>
            </CardHeader>
            <BosonCardContent>
                <Flex vertical=true gap=orbital::components::SpacingSize::Size120.flex_gap()>
                    <div data-testid="run-error-note">
                        <MessageBar intent=MessageBarIntent::Info>{ERROR_DATA_NOTE}</MessageBar>
                    </div>
                    <div data-testid="run-error-text">
                        <MessageBar intent=MessageBarIntent::Error>
                            <Text font=TextFont::Monospace>{message}</Text>
                        </MessageBar>
                    </div>
                </Flex>
            </BosonCardContent>
        </Card>
    }
}
