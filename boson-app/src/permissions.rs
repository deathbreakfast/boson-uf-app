//! Permission manifest and resource kinds for the Boson operations app.
//!
//! Two layers of access:
//!
//! - [`BosonPermission::BosonAdmin`], the app-wide permission synced into the `boson` domain.
//!   The host's admin group holds it, and that group is the default owner of every task type
//!   and queue bundle.
//! - A permission bundle per task type ([`BOSON_TASK_TYPE_KIND`]) and per queue
//!   ([`BOSON_QUEUE_KIND`]). Server functions check the resource's action as the session actor
//!   through `uf_product::access::require`, and the pages show the shared Access card and
//!   denied states.
//!
//! | Kind | Resource id | Actions |
//! |------|-------------|---------|
//! | Task type | task name from `#[task]` | View (its jobs, runs and errors), Run (retry, cancel, drop), Edit (settings and limits), Maintain |
//! | Queue | pool name | View (numbers and jobs), Run, Edit (pause, resume), Delete (purge), Maintain |
//!
//! Task types and queues are declared in code, so their bundles are made at host boot by
//! `ensure_boson_resource_bundles` (SSR), with the host's admin group as default owner.

use uf_product::access::ResourceRef;
use uf_product_macros::UfPermissionManifest;

/// Admin permission for the Boson ops UI.
///
/// Synced into the `boson` domain. The host's admin group holds it; the dashboard's
/// cross-type panels still check it, and per-resource checks cover everything else.
#[allow(clippy::expl_impl_clone_on_copy)]
#[derive(UfPermissionManifest)]
#[permission_manifest(
    domain_key = "boson",
    domain_name = "Boson",
    domain_description = "Boson background-work administration"
)]
pub enum BosonPermission {
    /// Administer every task type and queue.
    #[permission(description = "Administer Boson job cancellation and task configuration")]
    BosonAdmin,
}

/// Resource kind prefix for one Boson task type.
pub const BOSON_TASK_TYPE_KIND: &str = "boson_task_type";

/// Resource kind prefix for one Boson queue (Gluon pool).
pub const BOSON_QUEUE_KIND: &str = "boson_queue";

/// What the lists show for a resource the reader holds nothing on.
pub const NO_ACCESS: &str = "No access";

/// A task type as the shared access views name it.
#[must_use]
pub fn task_type_ref(task_name: &str) -> ResourceRef {
    ResourceRef::new(BOSON_TASK_TYPE_KIND, task_name).noun("task type")
}

/// A queue as the shared access views name it.
#[must_use]
pub fn queue_ref(queue_name: &str) -> ResourceRef {
    ResourceRef::new(BOSON_QUEUE_KIND, queue_name).noun("queue")
}

/// The role label for the actions the reader holds on one resource, strongest first.
///
/// `Maintain` reads as Owner. A reader with nothing gets [`NO_ACCESS`].
#[must_use]
pub fn access_label(held: &[&str]) -> &'static str {
    [
        ("Maintain", "Owner"),
        ("Edit", "Can edit"),
        ("Run", "Can run"),
        ("View", "Can view"),
    ]
    .into_iter()
    .find(|(action, _)| held.contains(action))
    .map_or(NO_ACCESS, |(_, label)| label)
}

/// The resource named in a permission-denied server error, with the missing action.
///
/// `uf_product::access::require` fails with `permission_denied::{kind}.{id}.{action}`. Ids
/// may contain dots, so the kind is matched as a known prefix and the action is the last
/// segment. Returns `None` for any other error.
#[must_use]
pub fn denied_resource(error: &str) -> Option<(ResourceRef, String)> {
    const PREFIX: &str = "permission_denied::";
    let rest = &error[error.find(PREFIX)? + PREFIX.len()..];
    let rest = rest.split_whitespace().next()?;
    let (name, action) = rest.rsplit_once('.')?;
    if let Some(id) = name.strip_prefix(&format!("{BOSON_QUEUE_KIND}.")) {
        return Some((queue_ref(id), action.to_string()));
    }
    let id = name.strip_prefix(&format!("{BOSON_TASK_TYPE_KIND}."))?;
    Some((task_type_ref(id), action.to_string()))
}

#[cfg(feature = "ssr")]
pub use bundles::{
    ensure_boson_resource_bundles, BundleSetupError, EnsuredBundles, BOSON_QUEUE, BOSON_TASK_TYPE,
};

#[cfg(feature = "ssr")]
mod bundles {
    use gauge::resource_permissions::{
        ensure_resource_permission_bundle_for_group, KindDefaultGroups, ResourceAction,
        ResourceKindDescriptor, ResourcePermissionError, UmbrellaPolicy,
    };

    use super::{BOSON_QUEUE_KIND, BOSON_TASK_TYPE_KIND};

    /// Gauge kind for one task type: View, Run, Edit, Maintain.
    pub const BOSON_TASK_TYPE: ResourceKindDescriptor = ResourceKindDescriptor {
        prefix: BOSON_TASK_TYPE_KIND,
        display_label: "Boson task type",
        actions: &[
            ResourceAction::View,
            ResourceAction::Run,
            ResourceAction::Edit,
            ResourceAction::Maintain,
        ],
        umbrella: UmbrellaPolicy::KindWide,
        groups: KindDefaultGroups {
            creators: "boson.task_type.creators",
            viewers: "boson.task_type.viewers",
            editors: "boson.task_type.editors",
            operators: "boson.task_type.operators",
        },
        create_permission: "CreateBosonTaskTypes",
    };

    /// Gauge kind for one queue: View, Run, Edit, Delete, Maintain.
    pub const BOSON_QUEUE: ResourceKindDescriptor = ResourceKindDescriptor {
        prefix: BOSON_QUEUE_KIND,
        display_label: "Boson queue",
        actions: &[
            ResourceAction::View,
            ResourceAction::Run,
            ResourceAction::Edit,
            ResourceAction::Delete,
            ResourceAction::Maintain,
        ],
        umbrella: UmbrellaPolicy::KindWide,
        groups: KindDefaultGroups {
            creators: "boson.queue.creators",
            viewers: "boson.queue.viewers",
            editors: "boson.queue.editors",
            operators: "boson.queue.operators",
        },
        create_permission: "CreateBosonQueues",
    };

    /// How many bundles [`ensure_boson_resource_bundles`] ensured.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EnsuredBundles {
        /// Task type bundles.
        pub task_types: usize,
        /// Queue bundles.
        pub queues: usize,
    }

    /// Gauge refused or failed one bundle during the boot ensure.
    #[derive(Debug)]
    pub struct BundleSetupError {
        /// Kind prefix of the resource.
        pub kind: &'static str,
        /// Resource id.
        pub resource_id: String,
        /// Gauge's error (including a non-System `Valence`).
        pub source: ResourcePermissionError,
    }

    impl std::fmt::Display for BundleSetupError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "boson access setup: ensure {} bundle for {}: {}",
                self.kind, self.resource_id, self.source
            )
        }
    }

    impl std::error::Error for BundleSetupError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.source)
        }
    }

    /// Ensure a permission bundle for every task type and queue the host runs.
    ///
    /// Call at host boot after manifest sync, with a System `Valence` (boot starts as System;
    /// never call this from a request). Pass the registry's task names and every pool the
    /// host offers. `admin_group_id` is the group that holds `BosonAdmin`: it becomes each
    /// bundle's default owner and holds every action, so people with Boson admin today keep
    /// their access. Idempotent; a resource that leaves the code keeps its bundle until
    /// someone removes it in Gauge. A queue that shows up later without a bundle reads as
    /// "No access" to everyone but Super User until the next boot ensures it.
    ///
    /// # Errors
    ///
    /// [`BundleSetupError`] when Gauge refuses a bundle: `system` is not System,
    /// `admin_group_id` doesn't exist, or a write fails.
    pub async fn ensure_boson_resource_bundles<T, Q>(
        system: &valence::Valence,
        task_names: T,
        queue_names: Q,
        admin_group_id: &str,
    ) -> Result<EnsuredBundles, BundleSetupError>
    where
        T: IntoIterator,
        T::Item: AsRef<str>,
        Q: IntoIterator,
        Q::Item: AsRef<str>,
    {
        let mut ensured = EnsuredBundles {
            task_types: 0,
            queues: 0,
        };
        for name in task_names {
            let name = name.as_ref();
            ensure_resource_permission_bundle_for_group(
                system,
                BOSON_TASK_TYPE,
                name,
                &format!("Boson task type {name}"),
                &[],
                admin_group_id,
            )
            .await
            .map_err(|source| BundleSetupError {
                kind: BOSON_TASK_TYPE_KIND,
                resource_id: name.to_string(),
                source,
            })?;
            ensured.task_types += 1;
        }
        for name in queue_names {
            let name = name.as_ref();
            ensure_resource_permission_bundle_for_group(
                system,
                BOSON_QUEUE,
                name,
                &format!("Boson queue {name}"),
                &[],
                admin_group_id,
            )
            .await
            .map_err(|source| BundleSetupError {
                kind: BOSON_QUEUE_KIND,
                resource_id: name.to_string(),
                source,
            })?;
            ensured.queues += 1;
        }
        Ok(ensured)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_label_picks_the_strongest_role() {
        assert_eq!(access_label(&["View", "Run", "Maintain"]), "Owner");
        assert_eq!(access_label(&["View", "Run", "Edit"]), "Can edit");
        assert_eq!(access_label(&["Run", "View"]), "Can run");
        assert_eq!(access_label(&["View"]), "Can view");
        assert_eq!(access_label(&[]), NO_ACCESS);
    }

    #[test]
    fn refs_name_the_kind_and_noun() {
        let task = task_type_ref("send_invoice");
        assert_eq!(task.kind, BOSON_TASK_TYPE_KIND);
        assert_eq!(task.id, "send_invoice");
        assert_eq!(task.noun, "task type");
        let queue = queue_ref("render-pool");
        assert_eq!(queue.kind, BOSON_QUEUE_KIND);
        assert_eq!(queue.noun, "queue");
    }

    #[test]
    fn denied_resource_reads_task_types_and_queues() {
        let err = "error running server function: permission_denied::boson_task_type.e2e_echo.Run";
        let (resource, action) = denied_resource(err).expect("denied task type");
        assert_eq!(resource.kind, BOSON_TASK_TYPE_KIND);
        assert_eq!(resource.id, "e2e_echo");
        assert_eq!(action, "Run");

        let err = "permission_denied::boson_queue.render.pool-a.Delete";
        let (resource, action) = denied_resource(err).expect("denied queue");
        assert_eq!(resource.kind, BOSON_QUEUE_KIND);
        assert_eq!(resource.id, "render.pool-a");
        assert_eq!(action, "Delete");
    }

    #[test]
    fn denied_resource_ignores_other_errors() {
        assert!(denied_resource("permission_denied::BosonAdmin").is_none());
        assert!(denied_resource("Failed to cancel job").is_none());
        assert!(denied_resource("permission_denied::photon_topic.x.View").is_none());
    }

    #[cfg(feature = "ssr")]
    #[test]
    fn descriptors_carry_the_designed_actions() {
        use gauge::resource_permissions::{permission_name, ResourceAction};
        let name = permission_name(BOSON_TASK_TYPE, "e2e_echo", ResourceAction::Run);
        assert!(name.starts_with("boson_task_type."), "{name}");
        assert!(name.ends_with(".Run"), "{name}");
        assert!(!BOSON_TASK_TYPE.actions.contains(&ResourceAction::Delete));
        assert!(BOSON_QUEUE.actions.contains(&ResourceAction::Delete));
        assert!(BOSON_QUEUE.actions.contains(&ResourceAction::Run));
    }
}
