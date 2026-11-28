//! Per-resource access checks for task types and queues.
//!
//! Every check runs as the session actor through the host's
//! [`uf_product::access::ResourceAccessBackend`]. A host that wires none falls back to the
//! app-wide `BosonAdmin` permission, so it keeps today's all-or-nothing access (and the Access
//! card says access control isn't set up there).

use std::collections::HashMap;
use std::sync::Arc;

use leptos::prelude::ServerFnError;
use uf_product::access::{ResourceAccessBackend, ResourceRef};

use crate::permissions::{access_label, queue_ref, task_type_ref, NO_ACCESS};

use super::types::{QueueSummary, TaskSummary};

/// Require `action` on one resource as the session actor, or `BosonAdmin` on a host with no
/// access backend.
async fn require(resource: &ResourceRef, action: &str) -> Result<(), ServerFnError> {
    if uf_product::access::use_resource_access_backend().is_none() {
        return uf_product::permissions::require_permission(super::BOSON_ADMIN_PERMISSION).await;
    }
    uf_product::access::require(resource, action).await
}

/// Require `action` on one task type as the session actor.
///
/// # Errors
///
/// A `permission_denied::boson_task_type.{name}.{action}` error when the reader lacks it.
pub(super) async fn require_task_type(task_name: &str, action: &str) -> Result<(), ServerFnError> {
    require(&task_type_ref(task_name), action).await
}

/// Require `action` on one queue as the session actor.
///
/// # Errors
///
/// A `permission_denied::boson_queue.{name}.{action}` error when the reader lacks it.
pub(super) async fn require_queue(queue_name: &str, action: &str) -> Result<(), ServerFnError> {
    require(&queue_ref(queue_name), action).await
}

/// The reader's answers for one request, so a list asks once per resource and action.
pub(super) struct ReaderAccess {
    backend: Option<Arc<dyn ResourceAccessBackend>>,
    answers: HashMap<(String, String, &'static str), bool>,
    /// `BosonAdmin`, read once, for a host with no access backend.
    admin: Option<bool>,
}

impl ReaderAccess {
    /// Checks against the request's backend, or `BosonAdmin` when the host wires none.
    pub(super) fn from_context() -> Self {
        Self {
            backend: uf_product::access::use_resource_access_backend(),
            answers: HashMap::new(),
            admin: None,
        }
    }

    /// Whether the reader holds `action` on `resource`; a failed check reads as no.
    async fn can(&mut self, resource: &ResourceRef, action: &'static str) -> bool {
        let key = (resource.kind.clone(), resource.id.clone(), action);
        if let Some(answer) = self.answers.get(&key) {
            return *answer;
        }
        let answer = match &self.backend {
            None => {
                if self.admin.is_none() {
                    self.admin = Some(
                        uf_product::permissions::has_permission(super::BOSON_ADMIN_PERMISSION)
                            .await,
                    );
                }
                self.admin == Some(true)
            }
            Some(backend) => match backend.can(resource, action).await {
                Ok(can) => can,
                Err(e) => {
                    tracing::warn!(
                        kind = %resource.kind,
                        id = %resource.id,
                        action,
                        error = %e,
                        "boson access check failed"
                    );
                    false
                }
            },
        };
        self.answers.insert(key, answer);
        answer
    }

    /// Whether the reader holds `action` on a task type.
    pub(super) async fn task_type(&mut self, task_name: &str, action: &'static str) -> bool {
        self.can(&task_type_ref(task_name), action).await
    }

    /// Whether the reader holds `action` on a queue.
    pub(super) async fn queue(&mut self, queue_name: &str, action: &'static str) -> bool {
        self.can(&queue_ref(queue_name), action).await
    }

    /// The reader's role label on one resource, checking the strongest action first.
    async fn label(&mut self, resource: &ResourceRef, actions: &[&'static str]) -> &'static str {
        for action in actions {
            if self.can(resource, action).await {
                return access_label(&[action]);
            }
        }
        NO_ACCESS
    }

    /// The reader's role label on a task type.
    pub(super) async fn task_type_label(&mut self, task_name: &str) -> &'static str {
        self.label(
            &task_type_ref(task_name),
            &["Maintain", "Edit", "Run", "View"],
        )
        .await
    }

    /// The reader's role label on a queue.
    pub(super) async fn queue_label(&mut self, queue_name: &str) -> &'static str {
        self.label(&queue_ref(queue_name), &["Maintain", "Edit", "Run", "View"])
            .await
    }

    /// Set each row's `access` and drop the numbers from rows the reader can't View.
    pub(super) async fn label_tasks(&mut self, tasks: &mut [TaskSummary]) {
        for task in tasks {
            task.access = self.task_type_label(&task.name).await.to_string();
            if !self.task_type(&task.name, "View").await {
                redact_task(task);
            }
        }
    }

    /// Set each row's `access` and drop the numbers from rows the reader can't View.
    pub(super) async fn label_queues(&mut self, queues: &mut [QueueSummary]) {
        for queue in queues {
            queue.access = self.queue_label(&queue.name).await.to_string();
            if !self.queue(&queue.name, "View").await {
                redact_queue(queue);
            }
        }
    }
}

/// A task row without its numbers; the name, queue, owner and signature (declared in code)
/// stay.
pub(super) const fn redact_task(task: &mut TaskSummary) {
    task.jobs_queued = 0;
    task.runs_total = 0;
    task.success_rate_pct = None;
}

/// A queue row with only its name, owner and state left.
pub(super) fn redact_queue(queue: &mut QueueSummary) {
    queue.waiting = 0;
    queue.running = 0;
    queue.oldest_waiting_at = None;
    queue.done_last_hour = 0;
    queue.added_last_hour = 0;
    queue.paused_by = None;
    queue.paused_at = None;
}
