//! Process-local overrides for `boson-uf-app-e2e` Playwright seeds.
//!
//! Enabled only with the `e2e-lab` Cargo feature (lab host). Production hosts
//! leave the feature off so setters are absent and overrides never apply.

#[cfg(all(feature = "ssr", feature = "e2e-lab"))]
use std::sync::atomic::{AtomicI8, Ordering};

/// `-1` = unset (use lepton-auth); `0` = force unverified; `1` = force verified.
#[cfg(all(feature = "ssr", feature = "e2e-lab"))]
static EMAIL_VERIFIED: AtomicI8 = AtomicI8::new(-1);

/// Set by `POST /api/test/seed-data` in boson-uf-app-e2e only.
#[cfg(all(feature = "ssr", feature = "e2e-lab"))]
pub fn set_email_verified_override(verified: Option<bool>) {
    let v = match verified {
        None => -1,
        Some(false) => 0,
        Some(true) => 1,
    };
    EMAIL_VERIFIED.store(v, Ordering::SeqCst);
}

#[cfg(all(feature = "ssr", feature = "e2e-lab"))]
pub(crate) fn email_verified_override() -> Option<bool> {
    match EMAIL_VERIFIED.load(Ordering::SeqCst) {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// Always unset when the lab feature is disabled (SSR only; hydrate has no callers).
#[cfg(all(feature = "ssr", not(feature = "e2e-lab")))]
#[must_use]
pub(crate) const fn email_verified_override() -> Option<bool> {
    None
}

/// Task type whose filtered Runs and Jobs list loads fail, as if the coordinator were down,
/// so the load-error spec can show Try again without touching other lists.
#[cfg(all(feature = "ssr", feature = "e2e-lab"))]
static FAIL_LIST_LOADS: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Fail list loads filtered to `task` until called again with `None`.
#[cfg(all(feature = "ssr", feature = "e2e-lab"))]
pub fn set_fail_list_loads(task: Option<String>) {
    if let Ok(mut slot) = FAIL_LIST_LOADS.lock() {
        *slot = task;
    }
}

/// Whether a list load filtered to `task` should fail (lab only).
#[cfg(all(feature = "ssr", feature = "e2e-lab"))]
pub(crate) fn list_load_fails(task: Option<&str>) -> bool {
    let Some(task) = task else { return false };
    FAIL_LIST_LOADS
        .lock()
        .is_ok_and(|slot| slot.as_deref() == Some(task))
}

/// Never fails when the lab feature is disabled.
#[cfg(all(feature = "ssr", not(feature = "e2e-lab")))]
#[must_use]
pub(crate) const fn list_load_fails(_task: Option<&str>) -> bool {
    false
}
