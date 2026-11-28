import type { Page } from "@playwright/test";
import { test, expect, seedAuth, uniquePool, waitForHydrated } from "./fixtures";

async function open(page: Page, path: string) {
  await page.goto(path, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
}

const taskPath = (name: string) => `/boson/tasks/${encodeURIComponent(name)}`;
const queuePath = (name: string) => `/boson/queues/${encodeURIComponent(name)}`;

/** Orbital components put their test id on an injected `<style>` too; skip that copy. */
const byTestId = (page: Page, id: string) => page.locator(`[data-testid="${id}"]:not(style)`);

/**
 * The "Your access" value on the Tasks list card of one task type. The Tasks table renders as
 * a list: each card shows its fields as label and value.
 */
const taskAccess = (page: Page, name: string) =>
  page
    .getByTestId(`data-table-list-card-${name}`)
    .locator(".orbital-data-table__list-card-field", { hasText: "Your access" })
    .locator(".orbital-data-table__list-card-value");

test.describe("e2e.access", () => {
  test("e2e.access.admin_owns_every_resource", async ({ page }) => {
    const pool = uniquePool("e2e-acc");
    const seeded = await seedAuth(page, "admin", { ops: { pool, queued: 1 } });
    await open(page, "/boson/tasks");
    await expect(taskAccess(page, seeded.fixtures.task_name)).toHaveText("Owner", {
      timeout: 60_000,
    });

    await open(page, "/boson/queues");
    const row = page.getByRole("row").filter({ has: page.getByTestId(`queue-row-${pool}`) });
    await expect(row.getByTestId("boson-your-access")).toHaveText("Owner", { timeout: 60_000 });
  });

  test("e2e.access.access_tab_shows_default_owner", async ({ page }) => {
    const pool = uniquePool("e2e-acc");
    const seeded = await seedAuth(page, "admin", { ops: { pool, queued: 1 } });
    for (const path of [taskPath(seeded.fixtures.task_name), queuePath(pool)]) {
      await open(page, path);
      await page.getByRole("tab", { name: "Access" }).click();
      await expect(page).toHaveURL(/\?tab=access$/);
      await expect(byTestId(page, "access-card")).toBeVisible({ timeout: 60_000 });
      await expect(byTestId(page, "access-card-error")).toHaveCount(0);
      await expect(byTestId(page, "access-card-rows")).toContainText("Boson admins", {
        timeout: 60_000,
      });
    }
    // The task type page also lists the team that can run it.
    await open(page, `${taskPath(seeded.fixtures.task_name)}?tab=access`);
    await expect(byTestId(page, "access-card-rows")).toContainText("Billing engineering", {
      timeout: 60_000,
    });
  });

  test("e2e.access.team_member_retries_failed_job", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "runner", { failedJob: true });
    const jobId = "e2e-failed-job-0087";

    await open(page, "/boson/tasks");
    await expect(taskAccess(page, seeded.fixtures.task_name)).toHaveText("Can run", {
      timeout: 60_000,
    });
    await expect(taskAccess(page, seeded.fixtures.settings_task)).toHaveText("No access");

    // Can run: no Settings tab, but the numbers show.
    await open(page, taskPath(seeded.fixtures.task_name));
    const tabs = page.getByTestId("task-detail-tabs");
    await expect(tabs.getByText("Overview", { exact: true })).toBeVisible({ timeout: 60_000 });
    await expect(tabs.getByText("Settings", { exact: true })).toHaveCount(0);
    await expect(page.getByTestId("task-detail-stats")).toBeVisible();

    await open(page, "/boson/jobs?tab=failed");
    const retry = page.getByTestId(`failed-retry-${jobId}`);
    await expect(retry).toBeVisible({ timeout: 60_000 });
    await retry.locator("button").click();
    await expect(page.getByTestId("retry-jobs-dialog")).toContainText("It runs as you.");
    await page.getByTestId("retry-jobs-confirm").locator("button").click();
    await expect(page.getByTestId("failed-jobs-success")).toContainText("Retried 1 job", {
      timeout: 60_000,
    });
  });

  test("e2e.access.outsider_sees_no_access", async ({ page }) => {
    const seeded = await seedAuth(page, "outsider", { failedJob: true });
    await open(page, "/boson/tasks");
    await expect(taskAccess(page, seeded.fixtures.task_name)).toHaveText("No access", {
      timeout: 60_000,
    });

    await open(page, taskPath(seeded.fixtures.task_name));
    const denied = page.getByTestId("access-denied");
    await expect(denied).toBeVisible({ timeout: 60_000 });
    await expect(denied).toHaveAttribute("data-action", "View");
    await expect(page.getByTestId("access-denied-message")).toContainText(
      `You need View on ${seeded.fixtures.task_name}`,
    );
    // The name, owner and arguments stay; the numbers and Settings don't.
    await expect(page.getByTestId("task-detail-owner")).toBeVisible();
    await expect(page.getByTestId("task-detail-arguments")).toBeVisible();
    await expect(page.getByTestId("task-detail-stats")).toHaveCount(0);
    await expect(
      page.getByTestId("task-detail-tabs").getByText("Settings", { exact: true }),
    ).toHaveCount(0);

    // No Retry or Drop on failed jobs of a type the outsider can't run.
    await open(page, "/boson/jobs?tab=failed");
    await expect(page.getByTestId("boson-failed-jobs-data-table")).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId("failed-retry-e2e-failed-job-0087")).toHaveCount(0);
  });

  test("e2e.access.request_approved_in_gauge", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "requester", { resetRequester: true });
    const task = seeded.fixtures.settings_task;

    await open(page, taskPath(task));
    const denied = page.getByTestId("access-denied");
    await expect(denied).toBeVisible({ timeout: 60_000 });
    await denied.getByTestId("access-denied-request").click();
    const fields = page.getByTestId("request-access-fields");
    await expect(fields).toBeVisible();
    await fields.getByRole("textbox").last().fill("On call for invoice syncs this week");
    await page.getByTestId("request-access-send").click();
    await expect(denied.getByTestId("access-denied-sent")).toBeVisible({ timeout: 60_000 });

    // An owner approves it in Gauge.
    const approved = await seedAuth(page, "admin", { approveRequests: true });
    expect(approved.approved).toBeGreaterThan(0);

    await seedAuth(page, "requester");
    await open(page, taskPath(task));
    await expect(page.getByTestId("task-detail-stats")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("access-denied")).toHaveCount(0);
  });
});
