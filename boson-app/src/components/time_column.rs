//! `DataTable` columns that show an RFC 3339 instant through Orbital `Timestamp`, with its hover
//! card (local, UTC and relative time).

use std::sync::Arc;

use leptos::prelude::*;
use orbital::primitives::{DataTableColumnDef, Timestamp, TimestampStyle};
use orbital_data::{DataRecord, DataValue};

/// A column whose cell renders the record's `field` (RFC 3339) as a `Timestamp` in `style`.
/// Blank values show nothing.
pub fn timestamp_column(
    field: &'static str,
    label: &'static str,
    style: TimestampStyle,
) -> DataTableColumnDef {
    let view = Arc::new(move |record: DataRecord| {
        let value = record
            .get(field)
            .map_or_else(String::new, DataValue::display_string);
        if value.is_empty() {
            return ().into_any();
        }
        view! { <Timestamp value=value style=style /> }.into_any()
    });
    DataTableColumnDef::new(field, label)
        .with_sortable(false)
        .with_filterable(false)
        .with_cell_view(view)
}
