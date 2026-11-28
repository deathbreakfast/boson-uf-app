//! Process-wide Valence + Higgs + MemQueue Boson for Playwright.
#![allow(dead_code)]

use std::sync::{Arc, Mutex, OnceLock};

use boson_backend_mem::MemQueueBackend;
use boson_coordinator::{BosonCoordinatorBackend, CoordinatorAdapter};
use boson_core::{
    ExecutionContext, Job, JobStatus, JsonExecutionContextFactory, QueueBackend, QueueRouter, Run,
    RunStatus,
};
use boson_runtime::{Boson, TaskDefaults, TaskDescriptor, TaskRegistry};
use chrono::Utc;
use gauge::manifest_sync::{
    sync_permission_manifests, PermissionDomainInput, PermissionInput, PermissionManifestInput,
};
use gauge::service;
use gauge::super_user::SUPER_USER_GROUP_NAME;
use higgs::actor_policy::external_actor_json_policy;
use higgs::{HiggsConfig, HiggsValenceFactory};
use valence::{
    register_backend_logical_names, router_key, Actor, DatabaseBackend, DatabaseRouter,
    InMemoryBackend, Model, RegisterBackendLogicalNamesOptions, RouterValenceFactory,
    RouterValenceFactoryConfig, Valence, ValenceFactory, MEM_ENGINE_ID, SQLITE_ENGINE_ID,
};

struct E2eState {
    router: Arc<DatabaseRouter>,
    higgs: Arc<HiggsConfig>,
    boson_backend: Arc<dyn BosonCoordinatorBackend>,
    queue: Arc<MemQueueBackend>,
    default_backend_key: String,
    fixtures: Mutex<FixtureIds>,
}

/// Stable fixture ids exposed to seed JSON / Playwright.
#[derive(Clone, Debug, Default)]
pub struct FixtureIds {
    pub task_name: String,
    pub job_id: String,
    pub run_id: String,
}

static E2E_STATE: OnceLock<Arc<E2eState>> = OnceLock::new();

/// Lab task name registered into the in-process registry.
pub const E2E_TASK_NAME: &str = "e2e_echo";

fn e2e_echo_invoke(
    _ctx: Box<dyn ExecutionContext>,
    _params: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = boson_core::Result<()>> + Send + 'static>> {
    Box::pin(async { Ok(()) })
}

/// Task type the Settings save spec edits, so its limits never block other specs' enqueues.
pub const E2E_SETTINGS_TASK_NAME: &str = "e2e_settings";

/// Task type the Limits spec fills to its "most at once" limit.
pub const E2E_LIMITED_TASK_NAME: &str = "e2e_limited";

/// Task type the Media platform team owns (Can edit), for the "yours first" specs.
pub const E2E_MEDIA_TASK_NAME: &str = "e2e_media_transcode";

/// Task type the Billing engineering team owns (Can edit), for the "yours first" specs.
pub const E2E_INVOICE_TASK_NAME: &str = "e2e_invoice_sync";

/// Queue the Media platform team owns (Can edit).
pub const E2E_MEDIA_POOL: &str = "e2e-media";

/// Signature of [`E2E_TASK_NAME`]: a description and one described argument, so the task
/// type page has an Arguments table to show.
const E2E_ECHO_SIGNATURE: &str = r#"{"description":"Echoes its input back. Used by the end-to-end tests.","properties":{"batch":{"type":"string","description":"The batch to echo, like batch-0042."}}}"#;

fn e2e_task_registry() -> TaskRegistry {
    let mut registry = TaskRegistry::new();
    let echo: &'static TaskDescriptor = Box::leak(Box::new(TaskDescriptor::with_defaults(
        E2E_TASK_NAME,
        e2e_echo_invoke,
        E2E_ECHO_SIGNATURE,
        0,
        TaskDefaults::standard(),
    )));
    registry.register(echo);
    for name in [
        E2E_SETTINGS_TASK_NAME,
        E2E_LIMITED_TASK_NAME,
        E2E_MEDIA_TASK_NAME,
        E2E_INVOICE_TASK_NAME,
    ] {
        let desc: &'static TaskDescriptor =
            Box::leak(Box::new(TaskDescriptor::new(name, e2e_echo_invoke)));
        registry.register(desc);
    }
    registry
}

struct HiggsFactory(RouterValenceFactory);

impl HiggsValenceFactory for HiggsFactory {
    fn build(&self, actor_json: &serde_json::Value) -> anyhow::Result<Valence> {
        self.0.build(actor_json).map_err(|e| anyhow::anyhow!("{e}"))
    }
}

fn prepare_env() {
    valence::deletion::register_noop_deletion_dispatcher_for_tests();
    valence::clear_for_test();
    // SAFETY: host boot only.
    unsafe {
        if std::env::var_os("VALENCE_OWNERSHIP_UNIFIED_FETCH").is_none() {
            std::env::set_var("VALENCE_OWNERSHIP_UNIFIED_FETCH", "0");
        }
    }
}

async fn seed_user(id: &str, email_verified: bool, valence: &Valence) {
    let now = Utc::now();
    let confirmed_at = email_verified.then_some(now);
    let user = lepton::generated::User::new(
        Some(lepton::generated::UserUserType::Person),
        Some("e2e-password-hash".to_string()),
        Some(lepton::generated::UserStatus::Active),
        None,
        None,
        confirmed_at,
        None,
        None,
        now,
        now,
    )
    .expect("build user");
    lepton::generated::User::upsert(id, user, valence, valence::use_!(r#"**Test:** Fixture **User** save for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#))
        .await
        .expect("upsert user");
}

async fn seed_super_user_with_member(system: &Valence, member_user_id: &str) {
    let super_group = gauge::generated::PermissionGroup::new(
        None,
        None,
        None,
        SUPER_USER_GROUP_NAME.to_string(),
        Some("super users".to_string()),
        Utc::now(),
        Utc::now(),
    )
    .expect("build super user group");
    let created =
        gauge::generated::PermissionGroup::upsert("super_user_group", super_group, system, valence::use_!(r#"**Test:** Fixture **Permission Group** save for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#))
            .await
            .expect("upsert super user group");

    let member = lepton::generated::User::get(member_user_id, system, valence::use_!(r#"**Test:** Fixture **User** load for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#))
        .await
        .expect("query member")
        .expect("member exists");
    let principal = gauge::generated::PermissionUserPrincipal::upsert(
        &format!("user:{member_user_id}"),
        gauge::generated::PermissionUserPrincipal::new(
            member.id().expect("member id").clone(),
            member_user_id.to_string(),
        )
        .expect("new principal"),
        system,
        valence::use_!(r#"**Test:** Fixture **Permission User Principal** save for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#),
    )
    .await
    .expect("upsert principal");
    created
        .relate_to_owner_record(
            principal.id().expect("principal id"),
            system,
            valence::use_!(r#"**Test:** Fixture **owner edge** write in `e2e_valence` so the suite can arrange Gauge super-user membership. CI and developers running the suite only."#),
        )
        .await
        .expect("relate super owner");
    created
        .relate_to_member_record(
            principal.id().expect("principal id"),
            system,
            valence::use_!(r#"**Test:** Fixture **member edge** write in `e2e_valence` so the suite can arrange Gauge super-user membership. CI and developers running the suite only."#),
        )
        .await
        .expect("relate super member");
}

async fn demote_admin_from_super_user(system: &Valence) {
    let Some(super_group) = gauge::generated::PermissionGroup::get("super_user_group", system, valence::use_!(r#"**Test:** Fixture **Permission Group** load for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#))
        .await
        .expect("get super user group")
    else {
        return;
    };
    let Some(principal) = gauge::generated::PermissionUserPrincipal::get("user:admin", system, valence::use_!(r#"**Test:** Fixture **Permission User Principal** load for `e2e_valence` so the suite can arrange and assert persistence behavior. CI and developers running the suite only."#))
        .await
        .expect("get admin principal")
    else {
        return;
    };
    let pid = principal.id().expect("principal id").clone();
    let _ = super_group
        .unrelate_from_member_record(
            &pid,
            system,
            valence::use_!(r#"**Test:** Fixture **member edge** remove in `e2e_valence` so the suite can demote a user from super-user after grants. CI and developers running the suite only."#),
        )
        .await;
    let _ = super_group
        .unrelate_from_owner_record(
            &pid,
            system,
            valence::use_!(r#"**Test:** Fixture **owner edge** remove in `e2e_valence` so the suite can demote a user from super-user after grants. CI and developers running the suite only."#),
        )
        .await;
}

fn boson_admin_manifest() -> PermissionManifestInput {
    PermissionManifestInput {
        app_id: "boson".into(),
        domains: vec![PermissionDomainInput {
            key: "boson".into(),
            name: "Boson".into(),
            description: "Boson background-work administration".into(),
            permissions: vec![PermissionInput {
                name: "BosonAdmin".into(),
                description: "Administer Boson job cancellation and task configuration".into(),
            }],
        }],
    }
}

/// Group that holds `BosonAdmin`; default owner of every task type and queue bundle.
pub const BOSON_ADMINS_GROUP_ID: &str = "boson.admins";

/// A team (mirrored the way Polaron mirrors teams) that holds Can run on [`E2E_TASK_NAME`].
pub const E2E_TEAM_GROUP_ID: &str = "e2e_team_billing";

/// Member of [`E2E_TEAM_GROUP_ID`]; holds nothing else.
pub const E2E_RUNNER_USER: &str = "runner";

/// A user with no access who asks for it in the request spec.
pub const E2E_REQUESTER_USER: &str = "requester";

/// Media platform team: owns (Can edit) [`E2E_MEDIA_TASK_NAME`] and [`E2E_MEDIA_POOL`].
pub const E2E_MEDIA_GROUP_ID: &str = "e2e_team_media";

/// Kenji, on the Media platform team.
pub const E2E_KENJI_USER: &str = "kenji";

/// Ada, on the Billing engineering team ([`E2E_TEAM_GROUP_ID`]), which owns (Can edit)
/// [`E2E_INVOICE_TASK_NAME`].
pub const E2E_ADA_USER: &str = "ada";

/// Every task type and queue the lab declares, for `owned_by` to check one by one. Queues the
/// suite makes on the fly join through [`ensure_queue_bundle`].
static KNOWN_QUEUES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Task types and queues of `kind` the lab knows about.
pub fn known_resource_ids(kind: &str) -> Vec<String> {
    if kind == boson_app::permissions::BOSON_QUEUE_KIND {
        let mut ids = vec![
            "global".to_string(),
            E2E_POOL.to_string(),
            E2E_MEDIA_POOL.to_string(),
        ];
        if let Ok(extra) = KNOWN_QUEUES.lock() {
            ids.extend(extra.iter().cloned());
        }
        ids
    } else {
        e2e_task_registry()
            .iter()
            .map(|d| d.name.to_string())
            .collect()
    }
}

/// The host's admin group: holds `BosonAdmin`, with `admin` as member and owner.
async fn seed_boson_admins_group(system: &Valence) {
    gauge::resource_permissions::ensure_group_granted_permission(
        system,
        BOSON_ADMINS_GROUP_ID,
        "Boson admins",
        "People who administer Boson",
        "BosonAdmin",
        &[],
    )
    .await
    .expect("seed Boson admins group");
    service::add_group_member_user(BOSON_ADMINS_GROUP_ID, "admin", system)
        .await
        .expect("add admin to Boson admins");
    // Gauge lets owners (not members) of the admin group manage each bundle's access.
    service::add_group_owner_user(BOSON_ADMINS_GROUP_ID, "admin", system)
        .await
        .expect("make admin an owner of Boson admins");
}

/// Ensure the bundle for a queue the suite makes on the fly, as host boot would for a
/// configured pool. Harness only: the seed endpoint runs as System from the start.
pub async fn ensure_queue_bundle(pool: &str) -> anyhow::Result<()> {
    if let Ok(mut known) = KNOWN_QUEUES.lock() {
        if !known.iter().any(|p| p == pool) {
            known.push(pool.to_string());
        }
    }
    boson_app::permissions::ensure_boson_resource_bundles(
        &e2e_system_valence(),
        std::iter::empty::<&str>(),
        [pool],
        BOSON_ADMINS_GROUP_ID,
    )
    .await?;
    Ok(())
}

/// Bundles for every task type and pool (as host boot does, as System), then the team with
/// [`E2E_RUNNER_USER`] in it and Can run on [`E2E_TASK_NAME`], granted by `owner` (a member
/// of the admin group) the way an owner does it from the Access card.
async fn seed_resource_access(system: &Valence, owner: &Valence, registry: &TaskRegistry) {
    use boson_app::permissions::{ensure_boson_resource_bundles, BOSON_TASK_TYPE};
    use gauge::resource_permissions::{roles::Role, set_principal_role};
    use gauge::types::PrincipalKind;

    let task_names: Vec<String> = registry.iter().map(|d| d.name.to_string()).collect();
    ensure_boson_resource_bundles(
        system,
        &task_names,
        ["global", E2E_POOL, E2E_MEDIA_POOL],
        BOSON_ADMINS_GROUP_ID,
    )
    .await
    .expect("ensure Boson resource bundles");

    service::create_managed_group(
        E2E_TEAM_GROUP_ID,
        "Billing engineering",
        "Team the end-to-end suite grants access to",
        Some("admin"),
        Some("polaron"),
        Some("billing"),
        system,
    )
    .await
    .expect("create the e2e team group");
    service::add_group_member_user(E2E_TEAM_GROUP_ID, E2E_RUNNER_USER, system)
        .await
        .expect("add runner to the team");
    set_principal_role(
        owner,
        BOSON_TASK_TYPE,
        E2E_TASK_NAME,
        (PrincipalKind::Group, E2E_TEAM_GROUP_ID),
        Role::Run,
    )
    .await
    .expect("the team can run the e2e task type");

    // Two teams that own work, for the "yours first" specs: Kenji's Media platform owns the
    // media task type and queue, Ada's Billing engineering owns the invoice task type.
    service::create_managed_group(
        E2E_MEDIA_GROUP_ID,
        "Media platform",
        "Team that owns the media task type and queue",
        Some("admin"),
        Some("polaron"),
        Some("media"),
        system,
    )
    .await
    .expect("create the media team group");
    service::add_group_member_user(E2E_MEDIA_GROUP_ID, E2E_KENJI_USER, system)
        .await
        .expect("add kenji to the media team");
    service::add_group_member_user(E2E_TEAM_GROUP_ID, E2E_ADA_USER, system)
        .await
        .expect("add ada to the billing team");
    set_principal_role(
        owner,
        BOSON_TASK_TYPE,
        E2E_MEDIA_TASK_NAME,
        (PrincipalKind::Group, E2E_MEDIA_GROUP_ID),
        Role::Edit,
    )
    .await
    .expect("the media team can edit the media task type");
    set_principal_role(
        owner,
        BOSON_TASK_TYPE,
        E2E_INVOICE_TASK_NAME,
        (PrincipalKind::Group, E2E_TEAM_GROUP_ID),
        Role::Edit,
    )
    .await
    .expect("the billing team can edit the invoice task type");
    set_media_queue_edit(owner, true)
        .await
        .expect("the media team can edit the media queue");
}

/// Give or take back the Media platform team's Can edit on [`E2E_MEDIA_POOL`], granted by
/// `owner` the way an owner does it from the Access card.
async fn set_media_queue_edit(owner: &Valence, edit: bool) -> anyhow::Result<()> {
    use boson_app::permissions::BOSON_QUEUE;
    use gauge::resource_permissions::{remove_principal, roles::Role, set_principal_role};
    use gauge::types::PrincipalKind;

    let team = (PrincipalKind::Group, E2E_MEDIA_GROUP_ID);
    if edit {
        set_principal_role(owner, BOSON_QUEUE, E2E_MEDIA_POOL, team, Role::Edit).await?;
    } else {
        remove_principal(owner, BOSON_QUEUE, E2E_MEDIA_POOL, team).await?;
    }
    Ok(())
}

/// Give or take back the media team's Can edit on its queue, as `admin`, for the denied-pause
/// spec.
pub async fn media_queue_edit(edit: bool) -> anyhow::Result<()> {
    let admin = e2e_system_valence().with_actor(Actor::User {
        user_id: "admin".to_string(),
    });
    set_media_queue_edit(&admin, edit).await
}

/// Jobs and runs of the two owned task types: 25 queued jobs and 25 succeeded runs each,
/// interleaved in time, so "yours first" puts a different 20 on page 1 for Kenji and Ada.
///
/// Jobs are dated two years ahead so they sort after every other spec's jobs; fixed ids make
/// repeat calls overwrite rather than add.
pub async fn seed_yours_first() -> anyhow::Result<()> {
    let queue = e2e_queue();
    let jobs_start = Utc::now() + chrono::Duration::days(730);
    let now = Utc::now();
    for i in 0..E2E_YOURS_FIRST_ROWS {
        for (n, (task, tag, pool)) in [
            (E2E_MEDIA_TASK_NAME, "media", E2E_MEDIA_POOL),
            (E2E_INVOICE_TASK_NAME, "inv", "global"),
        ]
        .into_iter()
        .enumerate()
        {
            let step = i64::try_from(i * 2 + n)?;
            let mut job = Job::new(
                task,
                serde_json::json!({"System": {"operation": "e2e_yours_first"}}),
                serde_json::json!({}),
                0,
                pool,
                0,
                None,
            );
            job.job_id = format!("e2e-yf-{tag}-{i:02}");
            job.created_at = jobs_start + chrono::Duration::seconds(step);
            job.status = JobStatus::Queued;
            queue.upsert_job(&job).await?;

            let mut run = Run::new(&job.job_id, task, 1);
            run.run_id = format!("e2e-yf-run-{tag}-{i:02}");
            run.status = RunStatus::Success;
            run.started_at = now - chrono::Duration::seconds(30 + step);
            run.finished_at = Some(run.started_at + chrono::Duration::seconds(1));
            run.duration_ms = Some(1_000);
            queue.upsert_run(&run).await?;
        }
    }
    Ok(())
}

/// Jobs and runs per owned task type that [`seed_yours_first`] upserts.
pub const E2E_YOURS_FIRST_ROWS: usize = 25;

/// Approve every pending access request `admin` may review, the way an owner does it in
/// Gauge's Requests page. Returns how many were approved.
pub async fn approve_pending_requests() -> anyhow::Result<usize> {
    use gauge::types::{PermissionRequestDecision, PermissionRequestDecisionInput};

    let admin = e2e_system_valence().with_actor(Actor::User {
        user_id: "admin".to_string(),
    });
    let pending = service::list_permission_requests_for_review(&admin).await?;
    let count = pending.len();
    for request in pending {
        service::decide_permission_request(
            PermissionRequestDecisionInput {
                request_id: request.id,
                decision: PermissionRequestDecision::Approve,
            },
            &admin,
        )
        .await?;
    }
    Ok(count)
}

/// Take back what [`E2E_REQUESTER_USER`] was given on [`E2E_SETTINGS_TASK_NAME`], so the
/// request spec starts from no access on every attempt. Approves anything still pending
/// first, so no stale request blocks a new one; runs as `admin`, an owner of the bundle.
pub async fn reset_requester_access() -> anyhow::Result<()> {
    use boson_app::permissions::BOSON_TASK_TYPE;
    use gauge::resource_permissions::remove_principal;
    use gauge::types::PrincipalKind;

    approve_pending_requests().await?;
    let admin = e2e_system_valence().with_actor(Actor::User {
        user_id: "admin".to_string(),
    });
    // Nothing to remove the first time; that's fine.
    let _ = remove_principal(
        &admin,
        BOSON_TASK_TYPE,
        E2E_SETTINGS_TASK_NAME,
        (PrincipalKind::User, E2E_REQUESTER_USER),
    )
    .await;
    Ok(())
}

async fn bootstrap_boson_fixtures(
    coordinator: &dyn BosonCoordinatorBackend,
    queue: &MemQueueBackend,
) -> anyhow::Result<FixtureIds> {
    let job_id = coordinator
        .enqueue(
            E2E_TASK_NAME,
            serde_json::json!({"System": {"operation": "e2e_seed"}}),
            serde_json::json!({}),
            None,
        )
        .await?;

    let mut run = Run::new(&job_id, E2E_TASK_NAME, 1);
    run.status = RunStatus::Success;
    run.finished_at = Some(Utc::now());
    run.duration_ms = Some(12);
    queue.upsert_run(&run).await?;

    Ok(FixtureIds {
        task_name: E2E_TASK_NAME.into(),
        job_id,
        run_id: run.run_id,
    })
}

/// Build shared Valence/Higgs/Boson once and seed baseline fixtures.
pub async fn init_e2e_valence() {
    if E2E_STATE.get().is_some() {
        return;
    }

    prepare_env();

    let backend: Arc<dyn DatabaseBackend> = Arc::new(InMemoryBackend::new());
    let mut router = DatabaseRouter::new();
    register_backend_logical_names(
        &mut router,
        Arc::clone(&backend),
        gauge::embedded_surreal::EMBEDDED_SURREAL_LOGICAL_NAMES,
        RegisterBackendLogicalNamesOptions {
            register_alias_engine_id: Some(SQLITE_ENGINE_ID),
        },
    );
    router.register(
        router_key(gauge::embedded_surreal::LOGICAL_NAME, SQLITE_ENGINE_ID),
        Arc::clone(&backend),
    );
    let router = Arc::new(router);
    let default_key = router_key(gauge::embedded_surreal::LOGICAL_NAME, MEM_ENGINE_ID);

    let system = Valence::builder()
        .database_router(Arc::clone(&router))
        .default_backend_key(default_key.clone())
        .with_actor(Actor::System {
            operation: "e2e_boson_host".into(),
        })
        .build()
        .expect("e2e Valence");

    seed_user("admin", true, &system).await;
    seed_user("outsider", true, &system).await;
    seed_user("unverified", false, &system).await;
    seed_user(E2E_RUNNER_USER, true, &system).await;
    seed_user(E2E_REQUESTER_USER, true, &system).await;
    seed_user(E2E_KENJI_USER, true, &system).await;
    seed_user(E2E_ADA_USER, true, &system).await;
    seed_super_user_with_member(&system, "admin").await;

    sync_permission_manifests(&system, &[boson_admin_manifest()])
        .await
        .expect("sync BosonAdmin manifest");

    let admin_ctx = system.with_actor(Actor::User {
        user_id: "admin".to_string(),
    });
    seed_boson_admins_group(&system).await;
    // Holds Boson admin (and so every bundle) for the email-gate sad paths.
    service::add_group_member_user(BOSON_ADMINS_GROUP_ID, "unverified", &system)
        .await
        .expect("add unverified to Boson admins");
    let registry = e2e_task_registry();
    seed_resource_access(&system, &admin_ctx, &registry).await;
    demote_admin_from_super_user(&system).await;

    let queue = Arc::new(MemQueueBackend::new());
    let queue_backend: Arc<dyn QueueBackend> = Arc::clone(&queue) as Arc<dyn QueueBackend>;
    QueueRouter::set_global(QueueRouter::with_default(Arc::clone(&queue_backend)));

    let boson = Arc::new(
        Boson::builder()
            .queue_backend_from_global()
            .execution_context_factory(JsonExecutionContextFactory)
            .registry(std::sync::Arc::new(registry))
            .without_worker()
            .build_manual()
            .expect("e2e Boson")
            .0,
    );
    boson_runtime::configure((*boson).clone());
    let boson_backend: Arc<dyn BosonCoordinatorBackend> =
        Arc::new(CoordinatorAdapter::new(Arc::clone(&boson)));

    let fixtures = bootstrap_boson_fixtures(boson_backend.as_ref(), queue.as_ref())
        .await
        .expect("bootstrap boson fixtures");

    let factory: Arc<dyn HiggsValenceFactory> = Arc::new(HiggsFactory(RouterValenceFactory::new(
        Arc::clone(&router),
        RouterValenceFactoryConfig::new(default_key.clone())
            .actor_json_policy(external_actor_json_policy()),
    )));
    // Lab host provides the coordinator via Leptos `provide_context` (see main.rs).
    // Skip HiggsConfig::boson to avoid boson-coordinator version skew with higgs/boson.
    let higgs = Arc::new(
        HiggsConfig::builder()
            .valence_factory_arc(factory)
            .build()
            .expect("e2e HiggsConfig"),
    );

    let state = Arc::new(E2eState {
        router,
        higgs,
        boson_backend,
        queue,
        default_backend_key: default_key,
        fixtures: Mutex::new(fixtures),
    });
    let _ = E2E_STATE.set(state);
}

fn state() -> Arc<E2eState> {
    E2E_STATE
        .get()
        .expect("init_e2e_valence must run first")
        .clone()
}

pub fn e2e_router() -> Arc<DatabaseRouter> {
    Arc::clone(&state().router)
}

pub fn e2e_higgs_config() -> Arc<HiggsConfig> {
    Arc::clone(&state().higgs)
}

pub fn e2e_boson_backend() -> Arc<dyn BosonCoordinatorBackend> {
    Arc::clone(&state().boson_backend)
}

/// Extra pool the E2E picker offers next to `global`.
pub const E2E_POOL: &str = "e2e-pool-a";

struct E2ePools;

impl boson_app::BosonPoolProvider for E2ePools {
    fn pools(&self) -> Vec<boson_app::GluonPoolPickRow> {
        vec![
            boson_app::GluonPoolPickRow {
                id: "global".to_string(),
                label: "global (default)".to_string(),
                detail: "Default pool every worker drains.".to_string(),
            },
            boson_app::GluonPoolPickRow {
                id: E2E_POOL.to_string(),
                label: E2E_POOL.to_string(),
                detail: "Pool a host pins dedicated Boson workers to.".to_string(),
            },
        ]
    }
}

pub fn e2e_pool_provider() -> Arc<dyn boson_app::BosonPoolProvider> {
    Arc::new(E2ePools)
}

pub fn e2e_fixtures() -> FixtureIds {
    state().fixtures.lock().expect("fixtures").clone()
}

pub fn store_fixtures(fixtures: FixtureIds) {
    *state().fixtures.lock().expect("fixtures") = fixtures;
}

pub fn e2e_queue() -> Arc<MemQueueBackend> {
    Arc::clone(&state().queue)
}

/// Re-enqueue a fresh queued job (and optional run) for isolated cancel specs.
pub async fn refresh_queue_job() -> anyhow::Result<FixtureIds> {
    let backend = e2e_boson_backend();
    let queue = e2e_queue();
    let mut fixtures = e2e_fixtures();
    let job_id = backend
        .enqueue(
            E2E_TASK_NAME,
            serde_json::json!({"System": {"operation": "e2e_refresh"}}),
            serde_json::json!({}),
            None,
        )
        .await?;
    fixtures.job_id = job_id;
    store_fixtures(fixtures.clone());
    let _ = queue;
    Ok(fixtures)
}

/// Jobs the jobs-table paging spec pages through: `e2e-page-000` ... `e2e-page-089`.
pub const E2E_PAGING_JOBS: usize = 90;

/// Upsert [`E2E_PAGING_JOBS`] jobs, alternating queued and failed, so the queued filter spans
/// three 20-row pages.
///
/// They are dated a year ahead so they sort after every job other specs enqueue (the queue
/// table lists oldest first), which keeps those specs' jobs on page 1. Fixed ids make repeat
/// calls overwrite rather than add.
pub async fn seed_paging_jobs() -> anyhow::Result<()> {
    let queue = e2e_queue();
    let start = Utc::now() + chrono::Duration::days(365);
    for i in 0..E2E_PAGING_JOBS {
        let mut job = Job::new(
            E2E_TASK_NAME,
            serde_json::json!({"System": {"operation": "e2e_paging"}}),
            serde_json::json!({}),
            0,
            "global",
            0,
            None,
        );
        job.job_id = format!("e2e-page-{i:03}");
        job.created_at = start + chrono::Duration::seconds(i64::try_from(i)?);
        job.status = if i % 2 == 0 {
            JobStatus::Queued
        } else {
            JobStatus::Failed
        };
        queue.upsert_job(&job).await?;
    }
    Ok(())
}

/// Failed jobs and runs one failed-jobs spec seeds; fresh ids on every call.
#[derive(Clone, Debug, serde::Serialize)]
pub struct FailedSeed {
    /// Failed job ids, oldest first.
    pub job_ids: Vec<String>,
    /// The failed run of each job, in the same order.
    pub run_ids: Vec<String>,
    /// Each job's arguments summary as the Jobs table shows it.
    pub args: Vec<String>,
}

/// Error every seeded failed run ends with, so Retry warns that it may repeat.
pub const E2E_FAILED_ERROR: &str = "Ledger API refused the token (401)";

/// Upsert two failed jobs that used all three attempts, each with its failed last run.
pub async fn seed_failed_jobs() -> anyhow::Result<FailedSeed> {
    let queue = e2e_queue();
    let mut seed = FailedSeed {
        job_ids: Vec::new(),
        run_ids: Vec::new(),
        args: Vec::new(),
    };
    for i in 0..2 {
        let batch = format!("batch-{}", &uuid_suffix()[..8]);
        let mut job = Job::new(
            E2E_TASK_NAME,
            serde_json::json!({"User": {"user_id": "admin"}}),
            serde_json::json!({"batch": batch}),
            0,
            "global",
            0,
            None,
        );
        job.status = JobStatus::Failed;
        job.attempt = 3;
        job.created_at = Utc::now() + chrono::Duration::milliseconds(i);
        queue.upsert_job(&job).await?;
        let mut run = Run::new(&job.job_id, E2E_TASK_NAME, 3);
        run.status = RunStatus::Failed;
        run.finished_at = Some(Utc::now());
        run.duration_ms = Some(30_000);
        run.error_message = Some(E2E_FAILED_ERROR.to_string());
        queue.upsert_run(&run).await?;
        seed.args.push(format!("batch {batch}"));
        seed.job_ids.push(job.job_id);
        seed.run_ids.push(run.run_id);
    }
    Ok(seed)
}

/// Runs the Runs filter spec looks for; fresh ids on every call.
#[derive(Clone, Debug, serde::Serialize)]
pub struct RunsSeed {
    /// A failed run that started just now.
    pub recent_failed_run_id: String,
    /// A successful run that started three days ago.
    pub old_run_id: String,
}

/// Upsert a failed run from now and a successful run from three days ago.
pub async fn seed_filter_runs() -> anyhow::Result<RunsSeed> {
    let queue = e2e_queue();
    let job_id = e2e_fixtures().job_id;
    let mut recent = Run::new(&job_id, E2E_TASK_NAME, 1);
    recent.status = RunStatus::Failed;
    recent.finished_at = Some(Utc::now());
    recent.duration_ms = Some(1_800);
    recent.error_message = Some("e2e filter run".to_string());
    queue.upsert_run(&recent).await?;
    let mut old = Run::new(&job_id, E2E_TASK_NAME, 1);
    old.status = RunStatus::Success;
    old.started_at = Utc::now() - chrono::Duration::days(3);
    old.finished_at = Some(old.started_at + chrono::Duration::seconds(2));
    old.duration_ms = Some(2_000);
    queue.upsert_run(&old).await?;
    Ok(RunsSeed {
        recent_failed_run_id: recent.run_id,
        old_run_id: old.run_id,
    })
}

fn uuid_suffix() -> String {
    Run::new("x", "x", 1).run_id.replace('-', "")
}

/// User id the failed-job fixture runs as; [`e2e_people_lookup`] names it.
pub const E2E_PERSON_ID: &str = "e2e-user-ada";

/// Failed job the run detail spec opens: three failed attempts, no attempts left.
pub const E2E_FAILED_JOB_ID: &str = "e2e-failed-job-0087";

/// Upsert a failed [`E2E_TASK_NAME`] job with three failed runs and return the last run's id.
/// Fixed ids make repeat calls overwrite rather than add.
pub async fn seed_failed_job() -> anyhow::Result<String> {
    let queue = e2e_queue();
    let now = Utc::now();
    let mut job = Job::new(
        E2E_TASK_NAME,
        serde_json::json!({"User": {"user_id": E2E_PERSON_ID}}),
        serde_json::json!({"batch": "batch-0042"}),
        1,
        "global",
        0,
        None,
    );
    job.job_id = E2E_FAILED_JOB_ID.to_string();
    job.created_at = now - chrono::Duration::minutes(10);
    job.status = JobStatus::Failed;
    job.attempt = 3;
    queue.upsert_job(&job).await?;

    let mut last_run_id = String::new();
    for attempt in 1..=3_i32 {
        let mut run = Run::new(E2E_FAILED_JOB_ID, E2E_TASK_NAME, attempt);
        run.run_id = format!("e2e-failed-run-000{attempt}");
        run.started_at = now - chrono::Duration::minutes(i64::from(8 - 2 * attempt));
        run.finished_at = Some(run.started_at + chrono::Duration::seconds(30));
        run.duration_ms = Some(30_000);
        run.status = RunStatus::Failed;
        run.error_message = Some(format!(
            "invoice sync failed on attempt {attempt}: ledger API returned 401 Unauthorized"
        ));
        queue.upsert_run(&run).await?;
        last_run_id = run.run_id;
    }
    Ok(last_run_id)
}

/// Jobs waiting for [`E2E_LIMITED_TASK_NAME`]; its "most at once" limit is set to this.
pub const E2E_LIMITED_ACTIVE: u32 = 2;

/// Put [`E2E_LIMITED_TASK_NAME`] at its limit: [`E2E_LIMITED_ACTIVE`] queued jobs and a
/// "most at once" limit of the same number.
pub async fn seed_limited_task() -> anyhow::Result<()> {
    let queue = e2e_queue();
    for i in 0..E2E_LIMITED_ACTIVE {
        let mut job = Job::new(
            E2E_LIMITED_TASK_NAME,
            serde_json::json!({"System": {"operation": "e2e_limited"}}),
            serde_json::json!({}),
            1,
            "global",
            0,
            None,
        );
        job.job_id = format!("e2e-limited-{i}");
        queue.upsert_job(&job).await?;
    }
    let backend = e2e_boson_backend();
    let mut config = backend.get_task_config(E2E_LIMITED_TASK_NAME).await?;
    config.rate_limit_policy.max_in_flight = E2E_LIMITED_ACTIVE;
    config.updated_at = Utc::now();
    backend.upsert_task_config(config).await?;
    Ok(())
}

struct E2ePeople;

impl boson_app::BosonPeopleLookup for E2ePeople {
    fn display_name(&self, user_id: &str) -> Option<String> {
        (user_id == E2E_PERSON_ID).then(|| "Ada Mensah".to_string())
    }
}

/// People lookup stub so the run page's "Started by" shows a name.
pub fn e2e_people_lookup() -> Arc<dyn boson_app::BosonPeopleLookup> {
    Arc::new(E2ePeople)
}

/// Jobs and runs the dashboard and Queues specs seed into one pool.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct OpsSeed {
    /// Pool the jobs go to; specs pass a unique name so parallel tests don't collide.
    pub pool: String,
    /// Queued jobs to add.
    #[serde(default)]
    pub queued: usize,
    /// Running jobs to add.
    #[serde(default)]
    pub running: usize,
    /// Failed jobs (waiting for Retry or Drop) to add.
    #[serde(default)]
    pub failed_jobs: usize,
    /// Failed runs started now, with one error message, to add.
    #[serde(default)]
    pub failed_runs: usize,
}

/// Error message on runs [`seed_ops`] marks failed.
pub const E2E_OPS_ERROR: &str = "Ledger API refused the token (401)";

/// Upsert the jobs and runs an [`OpsSeed`] asks for, all for [`E2E_TASK_NAME`]. The e2e host
/// runs no worker, so queued jobs stay queued.
pub async fn seed_ops(seed: &OpsSeed) -> anyhow::Result<()> {
    ensure_queue_bundle(&seed.pool).await?;
    let queue = e2e_queue();
    let statuses = std::iter::repeat_n(JobStatus::Queued, seed.queued)
        .chain(std::iter::repeat_n(JobStatus::Running, seed.running))
        .chain(std::iter::repeat_n(JobStatus::Failed, seed.failed_jobs));
    for status in statuses {
        let mut job = Job::new(
            E2E_TASK_NAME,
            serde_json::json!({"System": {"operation": "e2e_ops"}}),
            serde_json::json!({}),
            0,
            &seed.pool,
            0,
            None,
        );
        job.status = status;
        queue.upsert_job(&job).await?;
    }
    for _ in 0..seed.failed_runs {
        let mut job = Job::new(
            E2E_TASK_NAME,
            serde_json::json!({"System": {"operation": "e2e_ops"}}),
            serde_json::json!({}),
            0,
            &seed.pool,
            0,
            None,
        );
        job.status = JobStatus::Canceled;
        queue.upsert_job(&job).await?;
        let mut run = Run::new(&job.job_id, E2E_TASK_NAME, 1);
        run.status = RunStatus::Failed;
        run.finished_at = Some(Utc::now());
        run.duration_ms = Some(5);
        run.error_message = Some(E2E_OPS_ERROR.to_string());
        queue.upsert_run(&run).await?;
    }
    Ok(())
}

pub fn e2e_system_valence() -> Valence {
    Valence::builder()
        .database_router(e2e_router())
        .default_backend_key(state().default_backend_key.clone())
        .with_actor(Actor::System {
            operation: "e2e_seed".into(),
        })
        .build()
        .expect("system valence")
}
