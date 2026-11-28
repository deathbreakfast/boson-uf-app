import { test as base, expect, type Page } from "@playwright/test";

export type SeedAuthKind =
  | "anonymous"
  | "admin"
  | "outsider"
  | "unverified"
  /** Member of the e2e team, which holds Can run on the e2e task type only. */
  | "runner"
  /** No access; asks for it in the request spec. */
  | "requester";

export type SeedFixtures = {
  task_name: string;
  job_id: string;
  run_id: string;
  /** Last run of the seeded failed job (set when `failedJob` is requested). */
  failed_run_id: string | null;
  /** Task type the Settings save spec edits. */
  settings_task: string;
  /** Task type the Limits spec fills to its limit. */
  limited_task: string;
  limited_active: number;
};

/** Two failed jobs (all three attempts used), with their failed runs. */
export type FailedSeed = {
  job_ids: string[];
  run_ids: string[];
  args: string[];
};

/** A failed run from just now and a successful run from three days ago. */
export type RunsSeed = {
  recent_failed_run_id: string;
  old_run_id: string;
};

/** Jobs and runs to add to one pool (the host runs no worker, so queued jobs stay queued). */
export type OpsSeed = {
  pool: string;
  queued?: number;
  running?: number;
  failed_jobs?: number;
  failed_runs?: number;
};

/** Error message on runs an `OpsSeed` marks failed. */
export const OPS_ERROR = "Ledger API refused the token (401)";

/** A pool name no other test uses. */
export function uniquePool(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}-${Math.floor(Math.random() * 1e6).toString(36)}`;
}

export async function seedAuth(
  page: Page,
  auth: SeedAuthKind,
  opts?: {
    refreshJob?: boolean;
    pagingJobs?: boolean;
    failedJobs?: boolean;
    filterRuns?: boolean;
    failedJob?: boolean;
    atLimit?: boolean;
    ops?: OpsSeed;
    /** Approve every pending access request as admin, as an owner would in Gauge. */
    approveRequests?: boolean;
    /** Take back the requester's access to the settings task type first. */
    resetRequester?: boolean;
  },
) {
  const res = await page.request.post("/api/test/seed-data", {
    data: {
      auth,
      refresh_job: opts?.refreshJob ?? false,
      paging_jobs: opts?.pagingJobs ?? false,
      failed_jobs: opts?.failedJobs ?? false,
      filter_runs: opts?.filterRuns ?? false,
      failed_job: opts?.failedJob ?? false,
      at_limit: opts?.atLimit ?? false,
      ops: opts?.ops ?? null,
      approve_requests: opts?.approveRequests ?? false,
      reset_requester: opts?.resetRequester ?? false,
    },
  });
  expect(res.ok()).toBeTruthy();
  return res.json() as Promise<{
    ok: boolean;
    auth: string;
    approved: number;
    fixtures: SeedFixtures;
    failed: FailedSeed | null;
    runs: RunsSeed | null;
  }>;
}

/** The last four characters of an id, as the tables show it after "…". */
export function shortId(id: string) {
  return `…${id.slice(-4)}`;
}

async function bootState(page: Page): Promise<"ready" | "error" | "loading"> {
  return page.evaluate(() => {
    const html = document.documentElement;
    if (html.getAttribute("data-orbital-hydrated") === "true") {
      return "ready";
    }
    if (html.getAttribute("data-orbital-boot-state") === "error") {
      return "error";
    }
    return "loading";
  });
}

/**
 * CI evidence (gauge/uf-app ships): Orbital can set boot `error` from a
 * non-WASM `unhandledrejection` matching bare `fetch`, then refuse dismiss.
 * When wasm is complete, clear the error bit and re-invoke dismiss.
 */
async function clearFalsePositiveBootError(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const html = document.documentElement;
    if (html.getAttribute("data-orbital-hydrated") === "true") {
      return true;
    }
    if (html.getAttribute("data-orbital-boot-state") !== "error") {
      return false;
    }
    const progress = (
      window as unknown as {
        __orbitalBootProgress?: { steps?: { wasm?: string } };
        __orbitalBootDismissOverlay?: () => void;
      }
    );
    const wasmComplete =
      progress.__orbitalBootProgress?.steps?.wasm === "complete" ||
      document.querySelectorAll(".orbital-boot-step--complete").length >= 4;
    const shellReady = !!document.querySelector("main");
    if (!wasmComplete || !shellReady) {
      return false;
    }
    html.removeAttribute("data-orbital-boot-state");
    if (typeof progress.__orbitalBootDismissOverlay === "function") {
      progress.__orbitalBootDismissOverlay();
    }
    if (html.getAttribute("data-orbital-hydrated") !== "true") {
      html.setAttribute("data-orbital-hydrated", "true");
      document.getElementById("orbital-boot-overlay")?.remove();
    }
    return true;
  });
}

/**
 * Wait for Orbital hydrate. On terminal boot `error`, wait for wasm under a
 * false-positive error before reloading. Never reload while `loading`.
 */
export async function waitForHydrated(page: Page, timeoutMs = 180_000) {
  const deadline = Date.now() + timeoutMs;
  let refreshes = 0;
  const maxRefreshes = 3;

  while (Date.now() < deadline) {
    const state = await bootState(page);
    if (state === "ready") {
      break;
    }
    if (state === "error") {
      if (await clearFalsePositiveBootError(page)) {
        break;
      }
      const waitUntil = Math.min(Date.now() + 30_000, deadline);
      let recovered = false;
      while (Date.now() < waitUntil) {
        await page.waitForTimeout(500);
        if ((await bootState(page)) === "ready") {
          recovered = true;
          break;
        }
        if (await clearFalsePositiveBootError(page)) {
          recovered = true;
          break;
        }
      }
      if (recovered) {
        break;
      }
      if (refreshes >= maxRefreshes) {
        break;
      }
      refreshes += 1;
      await page.waitForTimeout(1_500);
      await page.reload({ waitUntil: "load" });
      continue;
    }
    await page.waitForTimeout(500);
  }

  if ((await bootState(page)) === "error") {
    await clearFalsePositiveBootError(page);
  }

  await expect
    .poll(async () => bootState(page), { timeout: 10_000 })
    .toBe("ready");
  await expect(page.getByTestId("orbital-boot-overlay")).toHaveCount(0, {
    timeout: 60_000,
  });
  await expect(page.getByTestId("e2e-auth-bootstrap")).toBeAttached({
    timeout: 30_000,
  });
}

/** Higgs / server-fn deny surfaces as an Orbital error MessageBar. */
export async function expectMutationDenied(page: Page) {
  await expect(page.locator(".orbital-message-bar--error").first()).toBeVisible({
    timeout: 60_000,
  });
}

export const test = base;
export { expect };
