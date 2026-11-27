import type { Page } from "@playwright/test";
import { test, expect, seedAuth, waitForHydrated, uniquePool } from "./fixtures";

async function openQueue(page: Page, pool: string) {
  await page.goto(`/boson/queues/${pool}`, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
  await expect(page.getByTestId("boson-queue-detail")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("queue-detail-state")).toBeVisible({ timeout: 60_000 });
}

test.describe("pw-boson-queues", () => {
  test("pw-boson-queues-happy-list-shows-state", async ({ page }) => {
    const pool = uniquePool("e2e-q");
    await seedAuth(page, "admin", { ops: { pool, queued: 3 } });
    await page.goto("/boson/queues", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-queues")).toBeVisible({ timeout: 60_000 });
    await expect(
      page.getByText(
        "Each queue is a Gluon pool. Falling behind means jobs arrive faster than they finish.",
      ),
    ).toBeVisible();
    await expect(page.getByTestId("nav-boson-queues")).toBeAttached();
    const link = page.getByTestId(`queue-row-${pool}`);
    await expect(link).toBeVisible({ timeout: 60_000 });
    await expect(link).toHaveAttribute("href", `/boson/queues/${pool}`);
    await expect(page.getByTestId(`queue-state-${pool}`)).toContainText("Falling behind");
    await link.click();
    await expect(page).toHaveURL(new RegExp(`/boson/queues/${pool}$`));
    await expect(page.getByTestId("queue-detail-state")).toContainText("Falling behind", {
      timeout: 60_000,
    });
  });

  test("pw-boson-queues-happy-pause-keeps-new-job-waiting-then-resume", async ({ page }) => {
    const pool = uniquePool("e2e-q");
    await seedAuth(page, "admin", { ops: { pool, queued: 2 } });
    await openQueue(page, pool);
    await expect(page.getByTestId("queue-stat-waiting")).toContainText("2");

    await page.getByTestId("queue-pause-toggle").locator("button").click();
    await expect(page.getByTestId("queue-detail-state")).toContainText("Paused", {
      timeout: 30_000,
    });
    const bar = page.getByTestId("queue-paused-bar");
    await expect(bar).toContainText("Paused");
    await expect(bar).toContainText(
      "Waiting jobs stay put; running jobs finish. New jobs still join the queue.",
    );
    await expect(page.getByTestId("queue-pause-toggle")).toContainText("Resume");

    // A job enqueued into the paused queue joins it and stays waiting.
    await seedAuth(page, "admin", { ops: { pool, queued: 1 } });
    await openQueue(page, pool);
    await expect(page.getByTestId("queue-detail-state")).toContainText("Paused");
    await expect(page.getByTestId("queue-stat-waiting")).toContainText("3");
    await expect(page.getByTestId("queue-stat-running")).toContainText("0");
    const jobs = page.getByTestId("queue-jobs-table");
    await expect(jobs.getByText("Queued", { exact: true })).toHaveCount(3);

    await page.getByTestId("queue-pause-toggle").locator("button").click();
    await expect(page.getByTestId("queue-detail-state")).toContainText("Falling behind", {
      timeout: 30_000,
    });
    await expect(page.getByTestId("queue-paused-bar")).toHaveCount(0);
    await expect(page.getByTestId("queue-pause-toggle")).toContainText("Pause");
  });

  test("pw-boson-queues-happy-purge-removes-only-waiting", async ({ page }) => {
    const pool = uniquePool("e2e-q");
    await seedAuth(page, "admin", { ops: { pool, queued: 3, running: 1 } });
    await openQueue(page, pool);
    await expect(page.getByTestId("queue-stat-waiting")).toContainText("3");

    await page.getByTestId("queue-purge-open").locator("button").click();
    await expect(page.getByTestId("queue-purge-sentence")).toHaveText(
      `3 waiting jobs in ${pool} are removed and won't run. The 1 running job finishes. Whatever enqueued them isn't told.`,
    );
    const confirm = page.getByTestId("queue-purge-confirm").locator("button");
    await expect(confirm).toHaveText("Purge 3 jobs");
    await expect(confirm).toBeDisabled();
    const input = page.getByTestId("queue-purge-confirm-input").locator("input");
    await input.fill("wrong-name");
    await expect(confirm).toBeDisabled();
    await input.fill(pool);
    await expect(confirm).toBeEnabled();
    await confirm.click();

    await expect(page.getByTestId("queue-purge-sentence")).toHaveCount(0, { timeout: 30_000 });
    await expect(page.getByTestId("queue-stat-waiting")).toContainText("0", {
      timeout: 30_000,
    });
    await expect(page.getByTestId("queue-stat-running")).toContainText("1");
    await expect(page.getByTestId("queue-jobs-table").getByText("Running", { exact: true })).toHaveCount(1);
  });

  test("pw-boson-queues-sad-outsider-denied", async ({ page }) => {
    const pool = uniquePool("e2e-q");
    await seedAuth(page, "outsider", { ops: { pool, queued: 1 } });
    await page.goto("/boson/queues", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-queues")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByText("Couldn't load queues.")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId(`queue-row-${pool}`)).toHaveCount(0);
  });

  test("pw-boson-queues-sad-unknown-queue", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto(`/boson/queues/${uniquePool("e2e-missing")}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-queue-detail")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByText(/Couldn't load this queue\..*Queue not found/)).toBeVisible({
      timeout: 60_000,
    });
  });
});
