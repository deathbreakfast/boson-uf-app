//! Harness-only seed endpoint for Playwright.

use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::e2e_valence::{
    e2e_fixtures, refresh_queue_job, seed_failed_jobs, seed_filter_runs, seed_paging_jobs,
    store_fixtures,
};
use crate::gate_demos::{write_e2e_auth_kind, E2eAuthKind};

#[derive(Debug, Deserialize)]
pub struct SeedRequest {
    /// `anonymous` | `admin` | `outsider` | `unverified`
    #[serde(default = "default_auth")]
    pub auth: String,
    /// When true, enqueue a fresh queued job for cancel specs.
    #[serde(default)]
    pub refresh_job: bool,
    /// When true, upsert the jobs the jobs-table paging spec pages through.
    #[serde(default)]
    pub paging_jobs: bool,
    /// When true, upsert two failed jobs (with failed runs) for the failed-jobs spec.
    #[serde(default)]
    pub failed_jobs: bool,
    /// When true, upsert a recent failed run and a three-day-old run for the Runs spec.
    #[serde(default)]
    pub filter_runs: bool,
}

fn default_auth() -> String {
    E2eAuthKind::Anonymous.as_str().to_string()
}

pub async fn seed_data(
    session: tower_sessions::Session,
    Json(body): Json<SeedRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let kind = E2eAuthKind::parse(&body.auth);
    write_e2e_auth_kind(&session, kind)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Lab seam: skip lepton-auth Backend for task-config server fns.
    match kind {
        E2eAuthKind::Anonymous => boson_app::e2e_lab::set_email_verified_override(None),
        E2eAuthKind::Admin | E2eAuthKind::Outsider => {
            boson_app::e2e_lab::set_email_verified_override(Some(true));
        }
        E2eAuthKind::Unverified => {
            boson_app::e2e_lab::set_email_verified_override(Some(false));
        }
    }

    let mut fixtures = e2e_fixtures();
    if body.refresh_job {
        fixtures = refresh_queue_job().await.map_err(|e| {
            log::error!("seed refresh_job failed: {e:#}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        store_fixtures(fixtures.clone());
    }

    if body.paging_jobs {
        seed_paging_jobs().await.map_err(|e| {
            log::error!("seed paging_jobs failed: {e:#}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    let failed = if body.failed_jobs {
        Some(seed_failed_jobs().await.map_err(|e| {
            log::error!("seed failed_jobs failed: {e:#}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?)
    } else {
        None
    };
    let runs = if body.filter_runs {
        Some(seed_filter_runs().await.map_err(|e| {
            log::error!("seed filter_runs failed: {e:#}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?)
    } else {
        None
    };

    Ok(Json(serde_json::json!({
        "ok": true,
        "failed": failed,
        "runs": runs,
        "auth": kind.as_str(),
        "fixtures": {
            "task_name": fixtures.task_name,
            "job_id": fixtures.job_id,
            "run_id": fixtures.run_id,
        }
    })))
}
