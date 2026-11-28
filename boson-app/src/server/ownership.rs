//! What the reader's teams own, for the "yours first" caption above each list.

use leptos::prelude::*;

use super::types::ListOwnership;

#[cfg(feature = "ssr")]
use super::helpers::{require_session, trace_server_result};

/// What the reader's teams own of task types (`kind` = `task_types`) or queues (`queues`),
/// with the team names the list caption shows.
#[uf_product_macros::server]
pub async fn get_list_ownership(
    /// `task_types` or `queues`; anything else reads as task types.
    kind: String,
) -> Result<ListOwnership, ServerFnError> {
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        let kind = if kind == "queues" {
            crate::permissions::BOSON_QUEUE_KIND
        } else {
            crate::permissions::BOSON_TASK_TYPE_KIND
        };
        Ok(super::access::Ownership::from_context()
            .list_ownership(kind)
            .await)
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result("get_list_ownership", &result, None, None, None);
    result
}
