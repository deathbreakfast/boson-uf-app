import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

const settingsPath = (name: string) =>
  `/boson/tasks/${encodeURIComponent(name)}?tab=settings`;
const field = (page: import("@playwright/test").Page, id: string) =>
  page.getByTestId(id).locator("input");

test.describe("pw-boson-task-config", () => {
  test("pw-boson-task-config-happy-redirect", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    const name = seeded.fixtures.task_name;
    await page.goto(`/boson/tasks/${encodeURIComponent(name)}/config`, {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page).toHaveURL(new RegExp(`/boson/tasks/${name}\\?tab=settings$`), {
      timeout: 60_000,
    });
    await expect(page.getByTestId("boson-task-config")).toBeVisible({ timeout: 60_000 });
  });

  test("pw-boson-task-config-sad-unverified-email", async ({ page }) => {
    const seeded = await seedAuth(page, "unverified");
    await page.goto(settingsPath(seeded.fixtures.task_name), {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-task-config")).toHaveCount(0);
    await expect(
      page.getByTestId("email-verification-required-empty-state"),
    ).toBeAttached({ timeout: 60_000 });
  });

  test("pw-boson-task-config-happy-admin-save", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    const path = settingsPath(seeded.fixtures.settings_task);
    await page.goto(path, { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(field(page, "task-config-max-attempts")).not.toHaveValue("", {
      timeout: 60_000,
    });
    await field(page, "task-config-max-attempts").fill("5");
    await field(page, "task-config-max-in-flight").fill("10");
    await expect(page.getByTestId("task-config-retry-sentence")).toContainText(
      "A job gives up after 5 attempts.",
    );
    await page.getByTestId("task-config-save").locator("button").click();
    await expect(page.getByTestId("task-config-saved")).toContainText("Saved.", {
      timeout: 30_000,
    });
    await expect(page.locator(".orbital-message-bar--error")).toHaveCount(0);

    await page.reload({ waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(field(page, "task-config-max-attempts")).toHaveValue("5", {
      timeout: 60_000,
    });
    await expect(field(page, "task-config-max-in-flight")).toHaveValue("10");
  });

  test("pw-boson-task-config-sad-decimal-attempts", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(settingsPath(seeded.fixtures.settings_task), {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(field(page, "task-config-max-attempts")).not.toHaveValue("", {
      timeout: 60_000,
    });
    await field(page, "task-config-max-attempts").fill("1.5");
    await expect(page.getByTestId("task-config-max-attempts-error")).toContainText(
      "Enter a whole number from 1 to 20.",
    );
    await expect(page.getByTestId("task-config-save").locator("button")).toBeDisabled();
  });

  test("pw-boson-task-config-sad-at-limit-warning", async ({ page }) => {
    const seeded = await seedAuth(page, "admin", { atLimit: true });
    const active = seeded.fixtures.limited_active;
    await page.goto(settingsPath(seeded.fixtures.limited_task), {
      waitUntil: "domcontentloaded",
    });
    await waitForHydrated(page);
    await expect(page.getByTestId("task-limits-at-limit")).toContainText(
      `At the limit: ${active} of ${active}`,
      { timeout: 60_000 },
    );
    await expect(page.getByTestId("task-limits-usage")).toContainText(
      `${active} of ${active} waiting or running now`,
    );
  });

  test("pw-boson-task-config-happy-host-pool", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    const path = settingsPath(seeded.fixtures.task_name);
    await page.goto(path, { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const pool = page.getByTestId("task-config-pool").locator("select");
    await expect(pool).toBeVisible({ timeout: 60_000 });
    await expect(pool.locator("option")).toHaveText(["global (default)", "e2e-pool-a"]);
    await pool.selectOption("e2e-pool-a");
    await page.getByTestId("task-config-save").locator("button").click();
    await expect(page.getByTestId("task-config-saved")).toBeVisible({ timeout: 30_000 });

    await page.goto(path, { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("task-config-pool").locator("select")).toHaveValue(
      "e2e-pool-a",
      { timeout: 60_000 },
    );
  });
});
