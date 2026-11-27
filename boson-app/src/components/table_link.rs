use leptos::prelude::*;
use orbital::primitives::Link;

/// Orbital Link for table cells and metadata grids.
#[component]
pub fn BosonTableLink(
    /// Link target.
    href: String,
    /// Child content rendered inside the component.
    children: Children,
) -> impl IntoView {
    view! {
        <Link href=href>
            {children()}
        </Link>
    }
}
