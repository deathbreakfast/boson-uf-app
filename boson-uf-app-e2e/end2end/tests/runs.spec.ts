import { test, expect, seedAuth, shortId, waitForHydrated } from "./fixtures";
import type { Page } from "@playwright/test";

async function search(page: Page, text: string) {
  const box = page
    .getByTestId("boson-runs-data-table")
    .getByTestId("data-table-quick-search")
    .locator("input");
  await box.fill(text);
}

async function pick(page: Page, testid: string, value: string) {
  const select = page.getByTestId(testid).locator("select");
  await expect(async () => {
    await select.selectOption(value);
    await expect(select).toHaveValue(value);
  }).toPass({ timeout: 10_000 });
}

test.describe("pw-boson-runs", () => {
  test("pw-boson-runs-happy-list-detail", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/boson/runs", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-runs")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("boson-runs-data-table")).toBeVisible({ timeout: 60_000 });
    await page.goto(`/boson/runs/${encodeURIComponent(seeded.fixtures.run_id)}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-run-detail")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByText(seeded.fixtures.run_id).first()).toBeVisible();
  });

  test("pw-boson-runs-happy-filters-in-url", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "admin", { filterRuns: true });
    const recent = seeded.runs!.recent_failed_run_id;
    await page.goto("/boson/runs", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const recentRow = page.getByTestId(`runs-row-${recent}`);
    await expect(recentRow).toBeVisible({ timeout: 60_000 });
    await expect(recentRow).toContainText(shortId(recent));
    await expect(page.getByTestId("runs-total")).toContainText(/\d+ runs? in the last 24 hours/);
    await expect(page.getByTestId("runs-time-filter").locator("select")).toHaveValue("24h");

    // Status and task go into the URL and survive a reload.
    await pick(page, "runs-status-filter", "failed");
    await expect(page).toHaveURL(/status=failed/);
    await pick(page, "runs-task-filter", "e2e_echo");
    await expect(page).toHaveURL(/task=e2e_echo/);
    await expect(recentRow).toBeVisible({ timeout: 60_000 });
    await page.reload({ waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("runs-status-filter").locator("select")).toHaveValue("failed");
    await expect(page.getByTestId(`runs-row-${recent}`)).toBeVisible({ timeout: 60_000 });

    // Succeeded leaves the failed run out.
    await pick(page, "runs-status-filter", "success");
    await expect(page).toHaveURL(/status=success/);
    await expect(page.getByTestId(`runs-row-${recent}`)).toHaveCount(0, { timeout: 60_000 });
  });

  test("pw-boson-runs-happy-search-last-characters-and-time", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "admin", { filterRuns: true });
    const old = seeded.runs!.old_run_id;
    const tail = old.slice(-6);

    // Three days ago is outside the default 24 hours: the bar says so and offers Clear.
    await page.goto("/boson/runs", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-runs-data-table")).toBeVisible({ timeout: 60_000 });
    await search(page, tail);
    const noMatch = page.getByTestId("runs-no-match");
    await expect(noMatch).toContainText(`No runs match “${tail}” in the last 24 hours.`, {
      timeout: 60_000,
    });

    // Last 7 days finds it by its last characters.
    await pick(page, "runs-time-filter", "7d");
    await expect(page).toHaveURL(/from=7d/);
    await expect(page.getByTestId(`runs-row-${old}`)).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("runs-total")).toContainText("1 run in the last 7 days");

    // From a URL, too.
    await page.goto("/boson/runs?from=24h", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await search(page, "zz9999");
    await expect(noMatch).toBeVisible({ timeout: 60_000 });
    await noMatch.getByRole("button", { name: "Clear filters" }).click();
    await expect(noMatch).toHaveCount(0, { timeout: 60_000 });
  });

  test("pw-boson-runs-sad-unknown-run", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/boson/runs/__boson_e2e_no_such_run__", {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-run-detail")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByText(/Run not found/i)).toBeVisible({ timeout: 60_000 });
  });
});
