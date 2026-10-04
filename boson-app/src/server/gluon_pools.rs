//! Pool picker for Boson task configuration (app boundary).
//!
//! Hosts that pin Boson workers to Pion pools provide an
//! `Arc<dyn boson_backend::BosonPoolProvider>` in request context. Without one
//! the picker offers only the default `global` pool.

use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::helpers::{offered_pools, require_email_verified, require_session, trace_server_result};
use super::types::GluonPoolPickRow;

/// Lists the pools a task can be routed to on this host.
#[uf_product_macros::server(permission = "BosonAdmin")]
pub async fn list_gluon_pools_for_boson_task_config() -> Result<Vec<GluonPoolPickRow>, ServerFnError>
{
    let result = async {
        let ctx = higgs::Higgs::from_request().await?;
        require_session(&ctx)?;
        require_email_verified().await?;
        Ok(offered_pools())
    }
    .await;
    #[cfg(feature = "ssr")]
    trace_server_result(
        "list_gluon_pools_for_boson_task_config",
        &result,
        None,
        None,
        None,
    );
    result
}
