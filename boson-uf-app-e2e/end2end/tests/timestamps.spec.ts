import type { Locator, Page } from "@playwright/test";
import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

const ISO = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/;

/** Hover a Timestamp and check its card's UTC row. */
async function expectUtcCard(page: Page, stamp: Locator) {
  await stamp.locator(".orbital-timestamp__trigger").first().hover();
  const utc = page.getByTestId("timestamp-card-utc").first();
  await expect(utc).toBeVisible({ timeout: 10_000 });
  await expect(utc).toContainText("UTC");
}

test.describe("pw-boson-timestamps", () => {
  test("pw-boson-timestamps-runs-table", async ({ page }) => {
    test.setTimeout(90_000);
    const seeded = await seedAuth(page, "admin", { filterRuns: true });
    const runId = seeded.runs!.recent_failed_run_id;
    await page.goto("/boson/runs", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const table = page.getByTestId("boson-runs-data-table");
    await expect(table.getByTestId(`runs-row-${runId}`)).toBeVisible({ timeout: 60_000 });
    const time = table.locator("time[datetime]").first();
    await expect(time).toBeVisible();
    expect(await time.getAttribute("datetime")).toMatch(ISO);
    // No hand-formatted "HH:MM" text stands in for a time.
    await expectUtcCard(page, table);
  });

  test("pw-boson-timestamps-run-detail", async ({ page }) => {
    test.setTimeout(90_000);
    const seeded = await seedAuth(page, "admin", { failedJob: true });
    await page.goto(`/boson/runs/${encodeURIComponent(seeded.fixtures.failed_run_id!)}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    const started = page.getByTestId("run-info-started");
    await expect(started.locator("time[datetime]")).toBeVisible({ timeout: 60_000 });
    expect(await started.locator("time").getAttribute("datetime")).toMatch(ISO);
    await expect(page.getByTestId("run-info-finished").locator("time[datetime]")).toBeVisible();
    await expectUtcCard(page, started);
  });
});
