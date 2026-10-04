import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

test.describe("pw-boson-task-config", () => {
  test("pw-boson-task-config-sad-unverified-email", async ({ page }) => {
    const seeded = await seedAuth(page, "unverified");
    await page.goto(
      `/boson/tasks/${encodeURIComponent(seeded.fixtures.task_name)}/config`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-task-config")).toHaveCount(0);
    await expect(
      page.getByTestId("email-verification-required-empty-state"),
    ).toBeAttached({ timeout: 60_000 });
  });

  test("pw-boson-task-config-happy-admin-save", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    await page.goto(
      `/boson/tasks/${encodeURIComponent(seeded.fixtures.task_name)}/config`,
      { waitUntil: "domcontentloaded" },
    );
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-task-config")).toBeVisible({ timeout: 60_000 });
    await expect(page.getByTestId("task-config-save")).toBeVisible({ timeout: 60_000 });
    await page.getByTestId("task-config-save").locator("button").click();
    // Save succeeds without an error MessageBar (navigate or stay on form).
    await expect(page.locator(".orbital-message-bar--error")).toHaveCount(0, {
      timeout: 30_000,
    });
  });

  test("pw-boson-task-config-happy-host-pool", async ({ page }) => {
    const seeded = await seedAuth(page, "admin");
    const configPath = `/boson/tasks/${encodeURIComponent(seeded.fixtures.task_name)}/config`;
    await page.goto(configPath, { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const pool = page.getByTestId("task-config-pool").locator("select");
    await expect(pool).toBeVisible({ timeout: 60_000 });
    await expect(pool.locator("option")).toHaveText(["global (default)", "e2e-pool-a"]);
    await pool.selectOption("e2e-pool-a");
    await page.getByTestId("task-config-save").locator("button").click();
    await expect(page.locator(".orbital-message-bar--error")).toHaveCount(0, {
      timeout: 30_000,
    });

    await page.goto(configPath, { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("task-config-pool").locator("select")).toHaveValue(
      "e2e-pool-a",
      { timeout: 60_000 },
    );
  });
});
