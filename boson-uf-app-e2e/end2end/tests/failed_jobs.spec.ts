import { test, expect, seedAuth, shortId, waitForHydrated } from "./fixtures";
import type { Page } from "@playwright/test";

async function openFailed(page: Page) {
  await page.goto("/boson/jobs?tab=failed", { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
  await expect(page.getByTestId("boson-failed-jobs-data-table")).toBeVisible({
    timeout: 60_000,
  });
}

test.describe("pw-boson-failed-jobs", () => {
  test("pw-boson-failed-jobs-happy-retry-two", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "admin", { failedJobs: true });
    const failed = seeded.failed!;
    await openFailed(page);
    await expect(page.getByText(/Failed jobs are kept \d+ days/)).toBeVisible();

    for (const id of failed.job_ids) {
      const row = page.getByTestId(`data-table-row-${id}`);
      await expect(row).toBeVisible({ timeout: 60_000 });
      await expect(row).toContainText("3 of");
      await expect(row).toContainText("Ledger API refused the token (401)");
      await row.getByRole("checkbox").click();
    }
    const bulk = page.getByTestId("failed-jobs-bulk-bar");
    await expect(bulk).toContainText("2 selected");
    await page.getByTestId("failed-bulk-retry").locator("button").click();

    const dialog = page.getByTestId("retry-jobs-dialog");
    await expect(dialog).toContainText("from attempt 1");
    await expect(dialog).toContainText("They run as you.");
    await expect(dialog).toContainText("Both failed with “Ledger API refused the token (401)”");
    await page.getByTestId("retry-jobs-confirm").locator("button").click();

    await expect(page.getByTestId("failed-jobs-success")).toContainText(
      "Retried 2 jobs. They're waiting in global; follow them under Active.",
      { timeout: 60_000 },
    );
    await expect(bulk).toHaveCount(0);

    // The two new jobs wait under Active with the same arguments.
    await page.getByTestId("jobs-tabs").getByRole("tab", { name: "Active" }).click();
    const active = page.getByTestId("boson-jobs-data-table");
    for (const args of failed.args) {
      await expect(active.getByText(args, { exact: true })).toBeVisible({ timeout: 60_000 });
    }
  });

  test("pw-boson-failed-jobs-happy-drop-keeps-runs", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "admin", { failedJobs: true });
    const failed = seeded.failed!;
    const jobId = failed.job_ids[0];
    await openFailed(page);
    await expect(page.getByTestId(`job-card-${jobId}`)).toBeVisible({ timeout: 60_000 });

    await page.getByTestId(`failed-drop-${jobId}`).locator("button").click();
    const dialog = page.getByTestId("drop-jobs-dialog");
    await expect(dialog).toContainText("won't run again");
    await expect(dialog).toContainText("Its runs stay in Runs.");
    await page.getByTestId("drop-jobs-confirm").locator("button").click();
    await expect(page.getByTestId("failed-jobs-success")).toContainText("Dropped 1 job.", {
      timeout: 60_000,
    });
    await expect(page.getByTestId(`job-card-${jobId}`)).toHaveCount(0, { timeout: 60_000 });
    // The other seeded job is still waiting for Retry or Drop.
    await expect(page.getByTestId(`job-card-${failed.job_ids[1]}`)).toBeVisible();

    // The dropped job's run stays in Runs.
    await page.goto(`/boson/runs?job=${encodeURIComponent(jobId)}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId(`runs-row-${failed.run_ids[0]}`)).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId(`runs-row-${failed.run_ids[0]}`)).toContainText(
      shortId(failed.run_ids[0]),
    );
  });
});
