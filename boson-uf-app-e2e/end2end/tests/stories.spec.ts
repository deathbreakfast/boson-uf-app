import type { Page } from "@playwright/test";
import { test, expect, seedAuth, shortId, waitForHydrated } from "./fixtures";

// One test per persona story the Boson rework changed. Each walks the story end to end the way
// the person would, through the pages and links, not straight to the last screen.

async function open(page: Page, path: string) {
  await page.goto(path, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
}

const RAW = /server function|permission_denied/;

test.describe("pw-boson-stories", () => {
  test("pw-boson-story-cancel-a-runaway-job", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "admin", { refreshJob: true });
    const jobId = seeded.fixtures.job_id;
    await open(page, "/boson/jobs");
    const table = page.getByTestId("boson-jobs-data-table");
    const cancel = page.getByTestId(`job-cancel-${jobId}`);
    await expect(cancel).toBeVisible({ timeout: 60_000 });
    // Who owns it and how long it has waited are on the row.
    await expect(table.getByText("Boson admins").first()).toBeVisible();
    await expect(table.locator("time[datetime]").first()).toBeVisible();

    await cancel.locator("button").click();
    await page.getByTestId("cancel-job-confirm").locator("button").click();
    await expect(page.getByTestId("jobs-cancel-success")).toContainText(
      `Canceled job ${shortId(jobId)}`,
      { timeout: 60_000 },
    );
    await expect(cancel).toHaveCount(0, { timeout: 60_000 });
  });

  test("pw-boson-story-trace-a-failed-run-back-to-its-task", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "admin", { failedJob: true });
    const runId = seeded.fixtures.failed_run_id!;
    const task = seeded.fixtures.task_name;

    await open(page, "/boson/runs?status=failed&from=7d");
    const row = page.getByTestId(`runs-row-${runId}`);
    await expect(row).toBeVisible({ timeout: 60_000 });
    await row.locator("a").click();
    await expect(page).toHaveURL(new RegExp(`/boson/runs/${runId}`), { timeout: 60_000 });
    await expect(page.getByTestId("run-error-text")).toContainText("401 Unauthorized", {
      timeout: 60_000,
    });
    await page.getByTestId("run-info-task").locator("a").click();
    await expect(page).toHaveURL(new RegExp(`/boson/tasks/${task}`), { timeout: 60_000 });
    await expect(page.getByTestId("task-detail-owner")).toContainText("Owner: Boson admins", {
      timeout: 60_000,
    });
  });

  test("pw-boson-story-retry-the-failed-invoice-syncs", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "admin", { failedJobs: true });
    const [first] = seeded.failed!.job_ids;
    await open(page, "/boson/jobs?tab=failed");
    const retry = page.getByTestId(`failed-retry-${first}`);
    await expect(retry).toBeVisible({ timeout: 60_000 });
    await expect(
      page.getByTestId("boson-failed-jobs-data-table").locator("time[datetime]").first(),
    ).toBeVisible();
    await retry.locator("button").click();
    await expect(page.getByTestId("retry-jobs-dialog")).toBeVisible();
    await page.getByTestId("retry-jobs-confirm").locator("button").click();
    await expect(page.getByTestId("failed-jobs-success")).toContainText("Retried 1 job", {
      timeout: 60_000,
    });
    await expect(page.getByTestId("failed-jobs-error")).toHaveCount(0);
  });

  test("pw-boson-story-tune-the-retry-policy-on-her-teams-task", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "ada");
    const task = seeded.fixtures.invoice_task;

    // Her team's task type leads the Tasks list, marked Yours.
    await open(page, "/boson/tasks");
    const card = page.getByTestId(`data-table-list-card-${task}`);
    await expect(card.getByTestId("boson-yours")).toBeVisible({ timeout: 60_000 });

    await open(page, `/boson/tasks/${encodeURIComponent(task)}?tab=settings`);
    const attempts = page.getByTestId("task-config-max-attempts").locator("input");
    await expect(attempts).not.toHaveValue("", { timeout: 60_000 });
    await attempts.fill("4");
    await expect(page.getByTestId("task-config-retry-sentence")).toContainText(
      "A job gives up after 4 attempts.",
    );
    await page.getByTestId("task-config-save").locator("button").click();
    await expect(page.getByTestId("task-config-saved")).toContainText("Saved.", {
      timeout: 30_000,
    });
    await expect(page.locator("body")).not.toContainText(RAW);
  });

  test("pw-boson-story-look-around-boson-without-bosonadmin", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "outsider", { refreshJob: true });

    await open(page, "/boson/tasks");
    await expect(page.getByTestId("boson-list-caption")).toHaveText(
      "Your teams own no task types here, so this shows everyone's.",
      { timeout: 60_000 },
    );
    await expect(page.getByTestId("boson-tasks-data-table").getByText("No access").first())
      .toBeVisible({ timeout: 60_000 });

    await open(page, "/boson/jobs");
    await expect(page.getByText(shortId(seeded.fixtures.job_id)).first()).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId(`job-cancel-${seeded.fixtures.job_id}`)).toHaveCount(0);

    await open(page, "/boson/queues");
    await expect(page.getByTestId("boson-queues-table")).toBeVisible({ timeout: 60_000 });
    await expect(page.locator("body")).not.toContainText(RAW);
  });
});
