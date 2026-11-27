import { test, expect, seedAuth, waitForHydrated, uniquePool, OPS_ERROR } from "./fixtures";

test.describe("pw-boson-dashboard", () => {
  test("pw-boson-dashboard-happy-four-stats", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/boson", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-dashboard")).toBeVisible({ timeout: 60_000 });
    await expect(
      page.getByTestId("boson-dashboard").getByText("Dashboard", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByText("What's stuck, what's failing, and whether each queue is keeping up."),
    ).toBeVisible();
    for (const [id, label, href] of [
      ["dashboard-stat-waiting", "Jobs waiting", "/boson/jobs?tab=active"],
      ["dashboard-stat-running", "Running", "/boson/jobs?tab=active"],
      ["dashboard-stat-failed", "Failed (24h)", "/boson/runs?status=failed"],
      ["dashboard-stat-oldest", "Oldest waiting", "/boson/queues"],
    ]) {
      const card = page.getByTestId(id);
      await expect(card).toContainText(label, { timeout: 60_000 });
      await expect(card.locator("a")).toHaveAttribute("href", href);
    }
    // Tasks count and Quick Links are gone.
    await expect(page.getByTestId("dashboard-stat-tasks")).toHaveCount(0);
    await expect(page.getByText("Quick Links")).toHaveCount(0);
  });

  test("pw-boson-dashboard-happy-needs-attention-links", async ({ page }) => {
    const pool = uniquePool("e2e-dash");
    const seeded = await seedAuth(page, "admin", {
      ops: { pool, queued: 2, failed_jobs: 2, failed_runs: 3 },
    });
    const task = seeded.fixtures.task_name;
    await page.goto("/boson", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const card = page.getByTestId("dashboard-needs-attention");
    await expect(card).toBeVisible({ timeout: 60_000 });
    await expect(card.getByTestId("needs-attention-all-clear")).toHaveCount(0);

    // The seeded pool has 2 added in the last hour, none done, and 2 waiting.
    const slow = card.locator('[data-testid^="needs-attention-line-"]', {
      hasText: `${pool} is falling behind`,
    });
    await expect(slow).toBeVisible({ timeout: 60_000 });
    await expect(slow).toContainText("Slow");
    await expect(slow.locator("a")).toHaveAttribute("href", `/boson/queues/${pool}`);

    const failedJobs = card.locator('[data-testid^="needs-attention-line-"]', {
      hasText: new RegExp(`\\d+ failed jobs? in ${task}`),
    });
    await expect(failedJobs).toBeVisible();
    await expect(failedJobs.locator("a")).toHaveAttribute(
      "href",
      `/boson/jobs?tab=failed&task=${task}`,
    );

    const failing = card.locator('[data-testid^="needs-attention-line-"]', {
      hasText: new RegExp(`${task} failed \\d+ times today`),
    });
    await expect(failing).toBeVisible();
    await expect(failing).toContainText(`Most failures: ${OPS_ERROR}`);
    await expect(failing.locator("a")).toHaveAttribute(
      "href",
      `/boson/runs?task=${task}&status=failed`,
    );

    // Worst first: queue lines come before failure lines.
    const titles = await card.locator('[data-testid^="needs-attention-line-"]').allInnerTexts();
    const firstFailed = titles.findIndex((t) => t.startsWith("Failed"));
    const lastSlow = titles.map((t) => t.startsWith("Slow")).lastIndexOf(true);
    expect(lastSlow).toBeLessThan(firstFailed);
  });

  test("pw-boson-dashboard-happy-failing-most-one-decimal", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", {
      ops: { pool: uniquePool("e2e-dash"), failed_runs: 1 },
    });
    await page.goto("/boson", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const card = page.getByTestId("dashboard-failing-most");
    await expect(card).toBeVisible({ timeout: 60_000 });
    // The lab registers one task type, so the table has one row.
    await expect(card.getByTestId(`failing-most-row-${seeded.fixtures.task_name}`)).toBeVisible({
      timeout: 60_000,
    });
    await expect(card).toContainText(/\d+\.\d%/);
    await expect(card).toContainText(OPS_ERROR);
    await expect(card.getByTestId("dashboard-failing-most-all-tasks")).toHaveAttribute(
      "href",
      "/boson/tasks",
    );
  });

  test("pw-boson-dashboard-happy-chart-bucket-opens-runs", async ({ page }) => {
    await seedAuth(page, "admin", { ops: { pool: uniquePool("e2e-dash"), failed_runs: 1 } });
    await page.goto("/boson", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const chart = page.getByTestId("dashboard-run-trend-chart");
    await expect(page.getByText("Pick a bar to see its runs.")).toBeVisible({ timeout: 60_000 });
    const bars = chart.locator("rect.orb-bar-mark");
    await expect(bars.first()).toBeAttached({ timeout: 60_000 });
    // Empty buckets draw zero-height rects; pick the tallest bar.
    const tallest = await bars.evaluateAll((els) => {
      const heights = els.map((e) => Number(e.getAttribute("height") ?? 0));
      return heights.indexOf(Math.max(...heights));
    });
    // The chart's hover layers sit over the bars, so send the click to the rect itself.
    await bars.nth(tallest).dispatchEvent("click");
    await expect(page).toHaveURL(/\/boson\/runs\?from=[^&]+&to=[^&]+$/, { timeout: 30_000 });
  });

  test("pw-boson-dashboard-sad-keeping-up-queue-has-no-line", async ({ page }) => {
    // Running work with nothing waiting keeps up, so it gets no Slow line.
    const pool = uniquePool("e2e-calm");
    await seedAuth(page, "admin", { ops: { pool, running: 1 } });
    await page.goto("/boson", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const card = page.getByTestId("dashboard-needs-attention");
    await expect(card).toBeVisible({ timeout: 60_000 });
    await expect(card
        .locator('[data-testid^="needs-attention-line-"], [data-testid="needs-attention-all-clear"]')
        .first()).toBeVisible({
      timeout: 60_000,
    });
    await expect(card).not.toContainText(pool);
  });
});
