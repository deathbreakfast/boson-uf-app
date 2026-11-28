import type { Page } from "@playwright/test";
import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

async function open(page: Page, path: string) {
  await page.goto(path, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
}

const RAW = /server function|permission_denied/;

test.describe("pw-boson-errors", () => {
  test("pw-boson-errors-load-error-keeps-filters-and-tries-again", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "admin", { failListLoads: "e2e_limited" });
    try {
      await open(page, "/boson/runs?task=e2e_limited&status=failed&from=7d");
      const error = page.getByTestId("boson-table-load-error");
      await expect(error).toContainText(
        "Couldn't load runs. Boson's coordinator didn't answer. Your filters are kept.",
        { timeout: 60_000 },
      );
      await expect(page.locator("body")).not.toContainText(RAW);
      await expect(page).toHaveURL(/task=e2e_limited/);
      await expect(page.getByTestId("runs-status-filter").locator("select")).toHaveValue("failed");

      // The coordinator is back: Try again sends the same request and the bar goes.
      await seedAuth(page, "admin");
      await page.getByTestId("boson-table-load-error-retry").locator("button").click();
      await expect(error).toHaveCount(0, { timeout: 60_000 });
      await expect(page.getByTestId("boson-runs-data-table")).toBeVisible();
      await expect(page).toHaveURL(/task=e2e_limited/);
      expect(seeded.ok).toBe(true);
    } finally {
      await seedAuth(page, "admin");
    }
  });

  test("pw-boson-errors-refused-pause-names-whats-missing", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "kenji", { yoursFirst: true, mediaQueueEdit: true });
    const pool = seeded.fixtures.media_pool;
    try {
      await open(page, `/boson/queues/${encodeURIComponent(pool)}`);
      const toggle = page.getByTestId("queue-pause-toggle");
      await expect(toggle).toBeVisible({ timeout: 60_000 });

      // An owner takes the team's Can edit back while the page is open.
      await seedAuth(page, "kenji", { mediaQueueEdit: false });
      await toggle.locator("button").click();
      const page_ = page.getByTestId("boson-queue-detail");
      await expect(page_).toContainText("You can't pause or resume this queue.", {
        timeout: 60_000,
      });
      await expect(page_).toContainText(`That needs Can edit on ${pool}.`);
      await expect(page.locator("body")).not.toContainText(RAW);
    } finally {
      await seedAuth(page, "kenji", { mediaQueueEdit: true });
    }
  });

  test("pw-boson-errors-no-raw-server-text-without-admin", async ({ page }) => {
    test.setTimeout(120_000);
    await seedAuth(page, "outsider");
    for (const path of ["/boson", "/boson/jobs", "/boson/runs", "/boson/queues", "/boson/tasks"]) {
      await open(page, path);
      await expect(page.locator("main")).toBeVisible({ timeout: 60_000 });
      // Give each page's loads time to settle before reading the text (polling keeps the
      // network busy, so there is no idle state to wait for).
      await page.waitForTimeout(2_000);
      await expect(page.locator("body")).not.toContainText(RAW);
    }
  });
});
