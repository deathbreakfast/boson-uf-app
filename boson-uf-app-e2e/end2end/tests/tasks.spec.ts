import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

const taskPath = (name: string) => `/boson/tasks/${encodeURIComponent(name)}`;

test.describe("pw-boson-tasks", () => {
  test("pw-boson-tasks-happy-list-columns", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto("/boson/tasks", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const table = page.getByTestId("boson-tasks-data-table");
    await expect(page.getByTestId("boson-tasks")).toBeVisible({ timeout: 60_000 });
    await expect(table).toBeVisible({ timeout: 60_000 });
    // The table renders as a list: each row shows its fields with their labels.
    const name = table.getByText(seeded.fixtures.task_name, { exact: true }).first();
    await expect(name).toBeVisible({ timeout: 60_000 });
    for (const label of ["Owner", "Queue", "Waiting", "Success rate"]) {
      await expect(table.getByText(label, { exact: true }).first()).toBeVisible();
    }
    await expect(table.getByText("Not set yet").first()).toBeVisible();
    // No raw signature JSON or `pool="…", priority=…` strings, and no per-row buttons.
    await expect(table.getByText(/pool="/)).toHaveCount(0);
    await expect(table.getByText(/"properties"/)).toHaveCount(0);
    await expect(table.getByRole("button", { name: "Configure" })).toHaveCount(0);

    await name.click();
    await expect(page).toHaveURL(new RegExp(`/boson/tasks/${seeded.fixtures.task_name}$`));
    await expect(page.getByTestId("boson-task-detail")).toBeVisible({ timeout: 60_000 });
  });

  test("pw-boson-tasks-happy-detail-overview", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    const name = seeded.fixtures.task_name;
    await page.goto(taskPath(name), { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-task-detail")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("task-detail-breadcrumb")).toContainText("Tasks");
    await expect(page.getByTestId("task-detail-owner")).toContainText("Owner:", {
      timeout: 60_000,
    });
    await expect(page.getByTestId("task-detail-owner")).toContainText("Echoes its input back.");
    await expect(page.getByTestId("task-detail-stats")).toContainText("Runs (24h)");
    await expect(page.getByTestId("task-detail-stats")).toContainText("Success rate (24h)");
    await expect(page.getByTestId("task-detail-settings-sentence")).toContainText(
      "Changed on",
    );
    const args = page.getByTestId("task-detail-arguments");
    await expect(args).toContainText("batch");
    await expect(args).toContainText("Text");
    await expect(page.getByTestId("task-detail-recent-runs")).toBeVisible();
    await expect(page.getByTestId("task-detail-all-runs").locator("a")).toHaveAttribute(
      "href",
      `/boson/runs?task=${name}`,
    );
    await expect(page.getByTestId("task-detail-waiting").locator("a")).toHaveAttribute(
      "href",
      `/boson/jobs?task=${name}`,
    );
    await expect(page.getByTestId("task-detail-failed-runs").locator("a")).toHaveAttribute(
      "href",
      `/boson/runs?task=${name}&status=failed`,
    );
  });

  test("pw-boson-tasks-happy-tab-switch-keeps-page", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    const name = seeded.fixtures.task_name;
    await page.goto(taskPath(name), { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const owner = page.getByTestId("task-detail-owner");
    await expect(owner).toBeVisible({ timeout: 60_000 });
    // Mark the header; a remount would drop the marker.
    await owner.evaluate((el) => el.setAttribute("data-e2e-marker", "kept"));

    const tabs = page.getByTestId("task-detail-tabs");
    await tabs.getByText("Access", { exact: true }).click();
    await expect(page).toHaveURL(/\?tab=access$/);
    await expect(page.getByTestId("task-detail-access")).toBeVisible();
    await tabs.getByText("Overview", { exact: true }).click();
    await expect(page.getByTestId("task-detail-stats")).toBeVisible();
    await expect(owner).toHaveAttribute("data-e2e-marker", "kept");
  });

  test("pw-boson-tasks-sad-outsider-no-settings-tab", async ({ page }) => {
    const seeded = await seedAuth(page, "outsider");
    await page.goto(taskPath(seeded.fixtures.task_name), { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const tabs = page.getByTestId("task-detail-tabs");
    await expect(tabs.getByText("Overview", { exact: true })).toBeVisible({ timeout: 60_000 });
    await expect(tabs.getByText("Settings", { exact: true })).toHaveCount(0);
  });

  test("pw-boson-tasks-sad-unknown-task", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/boson/tasks/__boson_e2e_no_such_task__", {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-task-detail")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("task-detail-not-found")).toContainText(
      "No task type called __boson_e2e_no_such_task__",
      { timeout: 60_000 },
    );
  });
});
