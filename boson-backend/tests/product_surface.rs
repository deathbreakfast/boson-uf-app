//! Product surface contracts for boson-app (sibling crate).
//!
//! Lives under `boson-backend` so CI can gate route/testid/auth/admin needles
//! without compiling Orbital/turf UI when host pins churn. Pattern matches
//! photon-uf-app `photon-backend/tests/product_surface.rs`, gauge
//! `gauge/tests/product_surface.rs`, and lepton-uf-app
//! `lepton-shell/tests/product_surface.rs`.

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read_app(rel: &str) -> String {
    let path = workspace_root().join("boson-app").join("src").join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn boson_routes_mount_happy_path() {
    let lib = read_app("lib.rs");
    for needle in [
        r#"path!("boson")"#,
        r#"path!("")"#,
        r#"path!("tasks")"#,
        r#"path!("tasks/:task_name")"#,
        r#"path!("tasks/:task_name/config")"#,
        r#"path!("jobs")"#,
        r#"path!("queue") view=BosonQueueRedirect"#,
        r#"path!("runs")"#,
        r#"path!("runs/:id")"#,
        "BosonLayoutRouteView",
        "id: \"boson\"",
        "route_path: \"/boson\"",
        "permission_manifest: permissions::BosonPermission",
    ] {
        assert!(
            lib.contains(needle),
            "BosonRoutes / uf_app missing `{needle}`"
        );
    }
}

#[test]
fn boson_routes_drop_leaf_sad_path() {
    let lib = read_app("lib.rs");
    for needle in [
        r#"path!("tasks/:task_name")"#,
        r#"path!("tasks/:task_name/config")"#,
        r#"path!("jobs")"#,
        r#"path!("runs/:id")"#,
    ] {
        assert!(
            lib.contains(needle),
            "removing `{needle}` drops a Boson ops funnel entry"
        );
    }
    assert!(
        !lib.contains("unimplemented!"),
        "BosonRoutes must not ship unimplemented placeholders"
    );
}

#[test]
fn uf_app_wrong_id_sad_path() {
    let lib = read_app("lib.rs");
    assert!(
        lib.contains("id: \"boson\""),
        "wrong uf_app id breaks Orbital host registration"
    );
    assert!(
        !lib.contains("id: \"boson-app\""),
        "uf_app id must stay `boson` (product route id), not crate name boson-app"
    );
}

#[test]
fn layout_auth_gate_and_nav_happy_path() {
    let layout = read_app("layout.rs");
    for needle in [
        "boson-app-root",
        "RequireAuthenticated",
        "Outlet",
        "nav-boson-dashboard",
        "nav-boson-tasks",
        "nav-boson-jobs",
        "nav-boson-runs",
        "AppBarUserMenu",
        "UnifiedFieldShellLayout",
    ] {
        assert!(
            layout.contains(needle),
            "BosonLayout missing contract `{needle}`"
        );
    }
}

#[test]
fn layout_drop_auth_guard_sad_path() {
    let layout = read_app("layout.rs");
    assert!(
        layout.contains("RequireAuthenticated") && layout.contains("<Outlet />"),
        "removing RequireAuthenticated opens /boson pages to anonymous sessions"
    );
}

#[test]
fn layout_missing_nav_sad_path() {
    let layout = read_app("layout.rs");
    for id in [
        "nav-boson-dashboard",
        "nav-boson-tasks",
        "nav-boson-jobs",
        "nav-boson-runs",
    ] {
        assert!(
            layout.contains(id),
            "dropping `{id}` breaks operator left-nav contract"
        );
    }
}

/// Body of `fn_name` in `src`, up to the next server function or end of file.
fn fn_body<'a>(src: &'a str, fn_name: &str) -> &'a str {
    let start = src
        .find(&format!("fn {fn_name}("))
        .unwrap_or_else(|| panic!("missing `{fn_name}`"));
    let rest = &src[start..];
    let end = rest[1..]
        .find("\n#[uf_product_macros::server")
        .map_or(rest.len(), |i| i + 1);
    &rest[..end]
}

#[test]
fn admin_actions_check_the_resource_happy_path() {
    let jobs = read_app("server/jobs.rs");
    let tasks = read_app("server/tasks.rs");
    let runs = read_app("server/runs.rs");
    let queues = read_app("server/queues.rs");

    assert!(
        fn_body(&jobs, "authorize_job_action").contains(r#"require_task_type(task_name, "Run")"#),
        "Cancel, Retry and Drop must check Run on the job's task type"
    );
    for fn_name in ["get_task_config", "update_task_config"] {
        assert!(
            fn_body(&tasks, fn_name).contains(r#"require_task_type(&task_name, "Edit")"#),
            "`{fn_name}` must check Edit on the task type"
        );
    }
    assert!(
        fn_body(&tasks, "get_task_active_jobs")
            .contains(r#"require_task_type(&task_name, "View")"#),
        "get_task_active_jobs must check View on the task type"
    );
    for fn_name in ["get_run", "get_run_detail"] {
        assert!(
            fn_body(&runs, fn_name).contains(r#"require_task_type(&run.task_name, "View")"#),
            "`{fn_name}` must check View on the run's task type"
        );
    }
    let queue_check = fn_body(&queues, "resource_action");
    assert!(
        queue_check.contains(r#"Self::View => "View""#)
            && queue_check.contains(r#"Self::Edit => "Edit""#)
            && queue_check.contains(r#"Self::Purge => "Delete""#),
        "queue actions must map to View, Edit (pause, resume) and Delete (purge)"
    );
}

#[test]
fn admin_actions_drop_resource_check_sad_path() {
    let access = read_app("server/access.rs");
    let require = fn_body(&access, "require");
    assert!(
        require.contains("use_resource_access_backend().is_none()")
            && require.contains("require_permission(super::BOSON_ADMIN_PERMISSION)")
            && require.contains("uf_product::access::require(resource, action)"),
        "checks must go through the access backend, and fall back to BosonAdmin only on a host without one"
    );
    let jobs = read_app("server/jobs.rs");
    let tasks = read_app("server/tasks.rs");
    let pools = read_app("server/gluon_pools.rs");
    let runs = read_app("server/runs.rs");
    let combined = format!("{jobs}\n{tasks}\n{pools}\n{runs}");
    assert!(
        !combined.contains(r#"permission = "GaugeAdmin""#)
            && !combined.contains(r#"permission = "PhotonAdmin""#),
        "Boson admin actions must not gate on GaugeAdmin or PhotonAdmin"
    );
}

#[test]
fn server_require_session_happy_path() {
    let helpers = read_app("server/helpers.rs");
    assert!(
        helpers.contains("fn require_session")
            && helpers.contains("Authentication is required")
            && helpers.contains("session_user_id()"),
        "helpers must fail closed without a session"
    );
    assert!(
        helpers.contains("Boson backend not in request context"),
        "missing Boson backend context must surface a typed ServerFnError message"
    );
    assert!(
        helpers.contains("fn require_email_verified")
            && helpers.contains("Email verification is required"),
        "helpers must fail closed without a verified email for task-config"
    );

    let dashboard = read_app("server/dashboard.rs");
    let jobs = read_app("server/jobs.rs");
    let tasks = read_app("server/tasks.rs");
    for (src, call_site) in [
        (dashboard.as_str(), "get_dashboard_stats"),
        (tasks.as_str(), "get_tasks"),
        (jobs.as_str(), "cancel_job"),
    ] {
        assert!(src.contains(call_site), "server missing `{call_site}`");
    }
}

#[test]
fn server_drop_require_session_on_get_tasks_sad_path() {
    let tasks = read_app("server/tasks.rs");
    let start = tasks.find("pub async fn get_tasks").expect("get_tasks");
    let body = &tasks[start..start + 350.min(tasks.len() - start)];
    assert!(
        body.contains("require_session(&ctx)?"),
        "get_tasks must call require_session before Boson IO"
    );

    let jobs = read_app("server/jobs.rs");
    for action in ["cancel_job", "retry_jobs", "drop_jobs"] {
        let body = fn_body(&jobs, action);
        assert!(
            body.contains("require_session(&ctx)?")
                && body.contains("authorize_job_action(&task_name, JobAction::"),
            "{action} must require a session and run the job action check before Boson writes"
        );
    }
}

#[test]
fn task_detail_hides_run_errors_happy_path() {
    let tasks = read_app("server/tasks.rs");
    let start = tasks
        .find("pub async fn get_task_detail")
        .expect("get_task_detail");
    let end = tasks[start..]
        .find("pub async fn get_task_active_jobs")
        .map_or(tasks.len(), |i| start + i);
    let body = &tasks[start..end];
    assert!(
        body.contains("error_message = None"),
        "get_task_detail is open to every signed-in user, so recent runs must not carry handler error text"
    );
}

#[test]
fn task_config_email_gate_happy_path() {
    let task_detail = read_app("pages/task_detail/mod.rs");
    assert!(
        task_detail.contains("requires_email_verification=true")
            && task_detail.contains("BosonTaskConfigPage"),
        "the Settings tab must require email verification"
    );
    let lazy = read_app("lazy_routes.rs");
    assert!(
        lazy.contains("BosonTaskConfigRedirect")
            && task_detail.contains("boson_task_settings_path"),
        "/config must redirect to the Settings tab"
    );

    let tasks = read_app("server/tasks.rs");
    let start = tasks
        .find("pub async fn get_task_config")
        .expect("get_task_config");
    let body = &tasks[start..start + 500.min(tasks.len() - start)];
    assert!(
        body.contains("require_email_verified().await?"),
        "get_task_config must mirror the UI email-verification gate"
    );
}

#[test]
fn task_config_drop_email_gate_sad_path() {
    let task_detail = read_app("pages/task_detail/mod.rs");
    assert!(
        task_detail.contains("requires_email_verification=true"),
        "dropping email verification on the Settings tab opens config to unverified sessions"
    );
    let tasks = read_app("server/tasks.rs");
    let start = tasks
        .find("pub async fn update_task_config")
        .expect("update_task_config");
    let body = &tasks[start..start + 750.min(tasks.len() - start)];
    assert!(
        body.contains("require_email_verified().await?"),
        "update_task_config must keep require_email_verified"
    );
}

#[test]
fn index_pages_testid_and_list_bindings_happy_path() {
    let dashboard = read_app("pages/dashboard/mod.rs");
    for needle in [
        "boson-dashboard",
        "get_dashboard_stats",
        "get_needs_attention",
        "get_failing_most",
    ] {
        assert!(
            dashboard.contains(needle),
            "BosonRootPage / dashboard missing `{needle}`"
        );
    }

    let tasks = read_app("pages/tasks/mod.rs");
    assert!(
        tasks.contains("boson-tasks"),
        "BosonTasksIndexPage missing boson-tasks testid"
    );

    let jobs = read_app("pages/jobs/mod.rs");
    for needle in ["boson-jobs", "cancel_job", "FailedJobsPanel"] {
        assert!(jobs.contains(needle), "BosonJobsPage missing `{needle}`");
    }
    let failed = read_app("pages/jobs/failed.rs");
    for needle in [
        "retry_jobs",
        "drop_jobs",
        "DataTableSelectionMode::Multiselect",
    ] {
        assert!(
            failed.contains(needle),
            "FailedJobsPanel missing `{needle}`"
        );
    }

    let runs = read_app("pages/runs/mod.rs");
    assert!(
        runs.contains("boson-runs"),
        "BosonRunsIndexPage missing boson-runs testid"
    );
}

#[test]
fn index_drop_dashboard_testid_sad_path() {
    let dashboard = read_app("pages/dashboard/mod.rs");
    assert!(
        dashboard.contains("data_testid=\"boson-dashboard\""),
        "dropping boson-dashboard breaks host / future Playwright parity"
    );
    let tasks = read_app("pages/tasks/mod.rs");
    assert!(
        tasks.contains("data_testid=\"boson-tasks\""),
        "dropping boson-tasks breaks host / future Playwright parity"
    );
    let jobs = read_app("pages/jobs/mod.rs");
    assert!(
        jobs.contains("data_testid=\"boson-jobs\""),
        "dropping boson-jobs breaks host / future Playwright parity"
    );
    let runs = read_app("pages/runs/mod.rs");
    assert!(
        runs.contains("data_testid=\"boson-runs\""),
        "dropping boson-runs breaks host / future Playwright parity"
    );
}

#[test]
fn detail_pages_testid_and_bindings_happy_path() {
    let task = read_app("pages/task_detail/mod.rs");
    for needle in ["boson-task-detail", "get_task"] {
        assert!(
            task.contains(needle),
            "BosonTaskDetailPage missing `{needle}`"
        );
    }

    let config = read_app("pages/task_config/mod.rs");
    for needle in [
        "boson-task-config",
        "get_task_config",
        "list_gluon_pools_for_boson_task_config",
    ] {
        assert!(
            config.contains(needle),
            "BosonTaskConfigPage missing `{needle}`"
        );
    }

    let run = read_app("pages/run_detail/mod.rs");
    for needle in ["boson-run-detail", "get_run"] {
        assert!(
            run.contains(needle),
            "BosonRunDetailPage missing `{needle}`"
        );
    }
}

#[test]
fn detail_pages_missing_bindings_sad_path() {
    let task = read_app("pages/task_detail/mod.rs");
    assert!(task.contains("get_task"), "task detail must bind get_task");
    let config = read_app("pages/task_config/mod.rs");
    assert!(
        config.contains("get_task_config"),
        "task config must bind get_task_config"
    );
    let run = read_app("pages/run_detail/mod.rs");
    assert!(run.contains("get_run"), "run detail must bind get_run");
    assert!(
        !task.contains("unimplemented!")
            && !config.contains("unimplemented!")
            && !run.contains("unimplemented!"),
        "detail pages must not ship unimplemented placeholders"
    );
}

#[test]
fn permission_manifest_boson_admin_happy_path() {
    let perms = read_app("permissions.rs");
    for needle in [
        "domain_key = \"boson\"",
        "BosonAdmin",
        "UfPermissionManifest",
    ] {
        assert!(
            perms.contains(needle),
            "BosonPermission manifest missing `{needle}`"
        );
    }
}

#[test]
fn protected_boson_host_matches_uf_app_happy_path() {
    let host =
        fs::read_to_string(workspace_root().join("examples/protected-boson-host/src/main.rs"))
            .expect("protected-boson-host main.rs");
    for needle in [
        "\"app_id\": \"boson\"",
        "\"route_path\": \"/boson\"",
        "\"auth_gate\": \"RequireAuthenticated\"",
        "\"admin_permission\": \"BosonAdmin\"",
        "dashboard_stats",
    ] {
        assert!(
            host.contains(needle),
            "protected-boson-host missing contract `{needle}`"
        );
    }
    let lib = read_app("lib.rs");
    assert!(
        lib.contains("id: \"boson\"") && lib.contains("route_path: \"/boson\""),
        "host inventory must stay aligned with uf_app!"
    );
    let layout = read_app("layout.rs");
    assert!(
        layout.contains("RequireAuthenticated"),
        "host auth_gate must stay aligned with BosonLayout guard"
    );
    let perms = read_app("permissions.rs");
    assert!(
        perms.contains("BosonAdmin"),
        "host admin_permission must stay aligned with BosonPermission"
    );
}

#[test]
fn lazy_routes_wire_pages_happy_path() {
    let lazy = read_app("lazy_routes.rs");
    for needle in [
        "BosonRootPage",
        "BosonTasksIndexPage",
        "BosonTaskDetailPage",
        "BosonTaskConfigRedirect",
        "BosonJobsPage",
        "BosonRunsIndexPage",
        "BosonRunDetailPage",
        "BosonQueuesPage",
        "BosonQueueDetailPage",
        "BosonLayout",
    ] {
        assert!(
            lazy.contains(needle),
            "lazy_routes missing page wire `{needle}`"
        );
    }
}

#[test]
fn ops_path_helpers_encode_segments_happy_path() {
    let failing = read_app("pages/dashboard/failing_most.rs");
    let queues = read_app("pages/queues/mod.rs");
    let task_detail = read_app("pages/task_detail/mod.rs");
    let task_actions = read_app("components/tasks_data_table/actions.rs");
    let tasks_table = read_app("components/tasks_data_table/mod.rs");
    let runs_cols = read_app("components/runs_data_table/columns.rs");
    let runs_table = read_app("components/runs_data_table/mod.rs");
    let queue = read_app("components/queue_data_table/mod.rs");
    let run_info = read_app("pages/run_detail/run_info_grid.rs");
    for (label, src) in [
        ("failing_most", failing.as_str()),
        ("queues", queues.as_str()),
        ("task_detail", task_detail.as_str()),
        ("task_actions", task_actions.as_str()),
        ("tasks_data_table", tasks_table.as_str()),
        ("runs_columns", runs_cols.as_str()),
        ("runs_data_table", runs_table.as_str()),
        ("queue_data_table", queue.as_str()),
        ("run_info_grid", run_info.as_str()),
    ] {
        assert!(
            src.contains("boson_backend::boson_")
                || src.contains("boson_task_path")
                || src.contains("boson_run_path")
                || src.contains("boson_task_config_path")
                || src.contains("boson_runs_job_filter_path"),
            "{label} must build detail hrefs via boson_backend path helpers"
        );
        assert!(
            !src.contains("crate::paths::task(")
                && !src.contains("crate::paths::run(")
                && !src.contains("crate::paths::tasks_config("),
            "{label} must not interpolate raw ids into orbital paths::*"
        );
    }
}

#[test]
fn ops_path_helpers_drop_encoding_sad_path() {
    let failing = read_app("pages/dashboard/failing_most.rs");
    let queues = read_app("pages/queues/mod.rs");
    assert!(
        failing.contains("boson_backend::boson_task_path"),
        "dropping boson_task_path reopens path-segment smuggling via task names"
    );
    assert!(
        queues.contains("boson_backend::boson_queue_path"),
        "dropping boson_queue_path reopens path-segment smuggling via queue names"
    );
    let runs_cols = read_app("components/runs_data_table/columns.rs");
    assert!(
        runs_cols.contains("boson_backend::boson_run_path"),
        "dropping boson_run_path reopens path-segment smuggling via run ids"
    );
    let queue = read_app("components/queue_data_table/mod.rs");
    assert!(
        queue.contains("boson_backend::boson_runs_job_filter_path"),
        "dropping boson_runs_job_filter_path reopens query-value smuggling via job ids"
    );
}

#[test]
fn jobs_lists_hide_details_without_view_sad_path() {
    let jobs = read_app("server/jobs.rs");
    for list in ["list_jobs_page", "list_jobs_datatable_page"] {
        assert!(
            fn_body(&jobs, list).contains("apply_job_access("),
            "{list} must blank job arguments and run errors where the reader can't View"
        );
    }
    let apply = fn_body(&jobs, "apply_job_access");
    assert!(
        apply.contains(r#"task_type(&job.task_name, "View")"#)
            && apply.contains("args_summary.clear()")
            && apply.contains("last_error = None"),
        "job details must stay behind View on the task type"
    );
}

#[test]
fn queue_endpoints_share_one_access_check_happy_path() {
    let queues = read_app("server/queues.rs");
    for fn_name in [
        "pub async fn list_queues",
        "pub async fn get_queue",
        "pub async fn pause_queue",
        "pub async fn resume_queue",
        "pub async fn purge_queue",
    ] {
        assert!(
            queues.contains(fn_name),
            "queues server missing `{fn_name}`"
        );
    }
    assert!(
        fn_body(&queues, "authorize_queue").contains("super::access::require_queue("),
        "queue endpoints must check the queue's action through authorize_queue"
    );
    let checks = queues
        .matches("authorize_queue(&ctx, &name, QueueAction::")
        .count();
    assert!(
        checks >= 3,
        "get, pause/resume and purge must each call authorize_queue (found {checks})"
    );
    assert!(
        fn_body(&queues, "list_queues").contains("label_queues("),
        "the queues list must label every row with the reader's access"
    );
}

#[test]
fn queue_routes_and_nav_happy_path() {
    let lib = read_app("lib.rs");
    assert!(
        lib.contains(r#"path!("queues")"#) && lib.contains(r#"path!("queues/:name")"#),
        "BosonRoutes must mount /boson/queues and /boson/queues/:name"
    );
    let layout = read_app("layout.rs");
    assert!(
        layout.contains("nav-boson-queues"),
        "layout must link to Queues"
    );
}

#[test]
fn dashboard_reads_with_pool_stats_or_run_errors_require_boson_admin_sad_path() {
    let dashboard = read_app("server/dashboard.rs");
    for fn_name in ["get_needs_attention", "get_failing_most"] {
        let start = dashboard
            .find(&format!("pub async fn {fn_name}"))
            .unwrap_or_else(|| panic!("dashboard missing `{fn_name}`"));
        let attr = dashboard[..start]
            .rfind("#[uf_product_macros::server")
            .expect("server attribute");
        assert!(
            dashboard[attr..start].contains(r#"permission = "BosonAdmin""#),
            "`{fn_name}` returns pool stats or run errors and must require BosonAdmin"
        );
    }
}
