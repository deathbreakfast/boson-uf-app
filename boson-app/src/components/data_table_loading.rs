use leptos::prelude::*;
use orbital::components::{
    Skeleton, SkeletonItem, SkeletonItemSize, Table, TableBody, TableCell, TableRow,
};

const REFETCH_SKELETON_ROWS: usize = 5;

/// Skeleton overlay content for DataTable server refetch (search, filter, cancel refresh).
///
/// Use inside `<DataTableLoadingView slot>` as a direct child of [`orbital::primitives::DataTable`].
#[component]
pub fn BosonDataTableRefetchSkeleton() -> impl IntoView {
    view! {
        <Skeleton>
            <div class="orbital-data-table__overlay-skeleton">
                {(0..REFETCH_SKELETON_ROWS)
                    .map(|_| {
                        view! {
                            <SkeletonItem size=Signal::from(SkeletonItemSize::S16) />
                        }
                    })
                    .collect_view()}
            </div>
        </Skeleton>
    }
}

/// Skeleton rows with one placeholder per column, for a DataTable's loading overlay.
///
/// Use inside `<DataTableLoadingView slot>` so the placeholder keeps the table's shape while
/// the toolbar stays usable.
#[component]
pub fn BosonDataTableSkeletonRows(
    /// Number of columns the table shows.
    columns: usize,
) -> impl IntoView {
    view! {
        <div data-testid="boson-table-skeleton">
            <Skeleton>
                <Table>
                    <TableBody>
                        {(0..REFETCH_SKELETON_ROWS)
                            .map(|_| {
                                view! {
                                    <TableRow>
                                        {(0..columns)
                                            .map(|_| {
                                                view! {
                                                    <TableCell>
                                                        <SkeletonItem size=Signal::from(SkeletonItemSize::S16) />
                                                    </TableCell>
                                                }
                                            })
                                            .collect_view()}
                                    </TableRow>
                                }
                            })
                            .collect_view()}
                    </TableBody>
                </Table>
            </Skeleton>
        </div>
    }
}
