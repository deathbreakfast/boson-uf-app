import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

test.describe("pw-boson-run-detail", () => {
  test("pw-boson-run-detail-happy-failed-run", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", { failedJob: true });
    const runId = seeded.fixtures.failed_run_id!;
    const task = seeded.fixtures.task_name;
    await page.goto(`/boson/runs/${encodeURIComponent(runId)}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-run-detail")).toBeVisible({ timeout: 60_000 });

    const crumbs = page.getByTestId("run-detail-breadcrumb");
    await expect(crumbs).toContainText("Runs");
    await expect(crumbs).toContainText(task, { timeout: 60_000 });
    await expect(crumbs).toContainText(`Run …${runId.slice(-4)}`);

    await expect(page.getByTestId("run-detail-attempt-line")).toContainText(
      "Attempt 3 of 3 · the job has no attempts left",
    );
    await expect(page.getByTestId("run-detail-retry").locator("button")).toBeEnabled();
    await expect(page.getByTestId("run-detail-drop").locator("button")).toBeEnabled();
    await expect(page.getByTestId("run-info-task").locator("a")).toHaveAttribute(
      "href",
      `/boson/tasks/${task}`,
    );
    await expect(page.getByTestId("run-info-job")).toContainText("batch batch-0042");
    await expect(page.getByTestId("run-info-queue").locator("a")).toHaveAttribute(
      "href",
      "/boson/jobs?pool=global",
    );
    await expect(page.getByTestId("run-info-started-by")).toHaveText("Ada Mensah");
    await expect(page.getByTestId("run-info-grid")).toContainText("30.0 s");

    const attempts = page.getByTestId("run-attempts");
    for (const n of [1, 2, 3]) {
      await expect(attempts).toContainText(`Attempt ${n} ·`);
    }
    await expect(page.getByTestId("run-error-note")).toContainText(
      "Boson keeps it 30 days.",
    );
    await expect(page.getByTestId("run-error-text")).toContainText("401 Unauthorized");
  });

  test("pw-boson-run-detail-happy-retry-job", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", { failedJob: true });
    const runId = seeded.fixtures.failed_run_id!;
    await page.goto(`/boson/runs/${encodeURIComponent(runId)}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await page.getByTestId("run-detail-retry").locator("button").click({ timeout: 60_000 });
    await page.getByTestId("run-detail-retry-confirm").locator("button").click();
    const retried = page.getByTestId("run-detail-retried");
    await expect(retried).toContainText("is waiting to run", { timeout: 30_000 });
    await expect(retried.locator("a")).toHaveAttribute("href", /\/boson\/runs\?job=/);
  });

  test("pw-boson-run-detail-sad-success-run-has-no-retry", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(`/boson/runs/${encodeURIComponent(seeded.fixtures.run_id)}`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("run-info-grid")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("run-detail-retry")).toHaveCount(0);
    await expect(page.getByTestId("run-info-started-by")).toHaveText("System");
  });
});
