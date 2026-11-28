//! The Owner cell on every Boson list: the owner, and a "Yours" badge on rows the reader's
//! teams own.

use std::sync::Arc;

use leptos::prelude::*;
use orbital::components::SpacingSize;
use orbital::primitives::{
    Badge, BadgeAppearance, BadgeColor, Body1, DataTableColumnDef, Flex, FlexAlign, FlexWrap,
};
use orbital_data::{DataRecord, DataValue};

/// Record field holding the owner label.
pub const OWNER_FIELD: &str = "owner";
/// Record field that is `"yes"` on rows the reader's teams own.
pub const YOURS_FIELD: &str = "yours";

/// The owner label with a brand-tint "Yours" badge when the reader's teams own the row.
#[component]
pub fn OwnerCell(
    /// Owner label ("Media platform", "Boson admins").
    #[prop(into)]
    owner: String,
    /// Whether the reader's teams own the row.
    yours: bool,
) -> impl IntoView {
    view! {
        <Flex align=FlexAlign::Center wrap=FlexWrap::Wrap gap=SpacingSize::Size80.flex_gap()>
            <Body1>{owner}</Body1>
            {yours.then(|| view! {
                <Badge
                    appearance=BadgeAppearance::Tint
                    color=BadgeColor::Brand
                    attr:data-testid="boson-yours"
                >
                    "Yours"
                </Badge>
            })}
        </Flex>
    }
}

/// The two record fields an [`owner_column`] reads.
pub fn owner_fields(owner: Option<&str>, yours: bool) -> [(String, DataValue); 2] {
    [
        (
            OWNER_FIELD.into(),
            DataValue::Text(
                owner
                    .unwrap_or(boson_backend::DEFAULT_OWNER_LABEL)
                    .to_string(),
            ),
        ),
        (
            YOURS_FIELD.into(),
            DataValue::Text(if yours { "yes" } else { "" }.into()),
        ),
    ]
}

/// Owner column showing an [`OwnerCell`] from the record's [`OWNER_FIELD`] and [`YOURS_FIELD`].
pub fn owner_column() -> DataTableColumnDef {
    let view = Arc::new(|record: DataRecord| {
        let text = |key: &str| {
            record
                .get(key)
                .map_or_else(String::new, DataValue::display_string)
        };
        view! { <OwnerCell owner=text(OWNER_FIELD) yours=!text(YOURS_FIELD).is_empty() /> }
            .into_any()
    });
    DataTableColumnDef::new(OWNER_FIELD, "Owner")
        .with_sortable(false)
        .with_filterable(false)
        .with_cell_view(view)
}
