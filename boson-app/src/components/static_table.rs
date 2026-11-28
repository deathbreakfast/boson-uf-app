use std::collections::HashMap;

use orbital::primitives::{DataRecord, DataTableRowModel, DataTableToolbarConfig};
use orbital_data::DataValue;

/// Toolbar for short, fully loaded tables: no search, filters, columns, pivot or export.
pub const fn no_toolbar() -> DataTableToolbarConfig {
    DataTableToolbarConfig {
        quick_search: false,
        filter_panel: false,
        column_picker: false,
        pivot: false,
        export_menu: false,
    }
}

/// A client-side table row keyed by `id` with one text cell per column.
pub fn row_model(id: &str, cells: &[(&str, String)]) -> DataTableRowModel {
    DataTableRowModel::from_text_cells(
        id.to_owned(),
        cells
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect::<HashMap<_, _>>(),
    )
}

/// A text cell's value, or an empty string.
pub fn text_cell(record: &DataRecord, key: &str) -> String {
    record
        .get(key)
        .map_or_else(String::new, DataValue::display_string)
}
