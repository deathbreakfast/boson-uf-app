import {
  test,
  expect,
  seedAuth,
  shortId,
  waitForHydrated,
} from "./fixtures";

test.describe("pw-boson-jobs", () => {
  test("pw-boson-jobs-happy-queue-redirects-to-active", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/boson/queue", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page).toHaveURL(/\/boson\/jobs(\?|$)/, { timeout: 60_000 });
    await expect(page.getByTestId("boson-jobs")).toBeVisible({ timeout: 60_000 });
    const tabs = page.getByTestId("jobs-tabs");
    await expect(tabs.getByRole("tab", { name: "Active" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    // The nav can start collapsed; the Jobs entry only needs to exist.
    await expect(page.getByTestId("nav-boson-jobs")).toBeAttached();
  });

  test("pw-boson-jobs-happy-cancel-asks-then-names-job", async ({ page }) => {
    test.setTimeout(90_000);
    const seeded = await seedAuth(page, "admin", { refreshJob: true });
    const jobId = seeded.fixtures.job_id;
    await page.goto("/boson/jobs", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const cancel = page.getByTestId(`job-cancel-${jobId}`);
    await expect(cancel).toBeVisible({ timeout: 60_000 });

    // Keep running closes the dialog and leaves the job alone.
    await cancel.locator("button").click();
    const dialog = page.getByTestId("cancel-job-dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText("won't");
    await expect(dialog).toContainText(`Job ${jobId}`);
    await page.getByRole("button", { name: "Keep running" }).click();
    await expect(dialog).toBeHidden();
    await expect(cancel).toBeVisible();

    await cancel.locator("button").click();
    await page.getByTestId("cancel-job-confirm").locator("button").click();
    const success = page.getByTestId("jobs-cancel-success");
    await expect(success).toContainText(`Canceled job ${shortId(jobId)}`, { timeout: 60_000 });
    await expect(success).toContainText("e2e_echo");
    // Canceled jobs leave Active.
    await expect(cancel).toHaveCount(0, { timeout: 60_000 });
  });

  test("pw-boson-jobs-sad-cancel-non-admin", async ({ page }) => {
    const seeded = await seedAuth(page, "outsider", { refreshJob: true });
    await page.goto("/boson/jobs", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-jobs")).toBeVisible({ timeout: 60_000 });
    // The job is listed, but without Run on its task type there is no Cancel.
    await expect(page.getByText(shortId(seeded.fixtures.job_id)).first()).toBeVisible({
      timeout: 60_000,
    });
    await expect(page.getByTestId(`job-cancel-${seeded.fixtures.job_id}`)).toHaveCount(0);
  });

  test("pw-boson-jobs-happy-tabs-keep-the-page", async ({ page }) => {
    await seedAuth(page, "admin");
    await page.goto("/boson/jobs", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    await expect(page.getByTestId("boson-jobs-data-table")).toBeVisible({ timeout: 60_000 });
    // Mark the page so a full reload would be noticed.
    await page.evaluate(() => {
      (window as unknown as { __jobsMark?: boolean }).__jobsMark = true;
    });
    const tabs = page.getByTestId("jobs-tabs");
    await tabs.getByRole("tab", { name: "Failed" }).click();
    await expect(page).toHaveURL(/tab=failed/);
    await expect(page.getByTestId("boson-failed-jobs-data-table")).toBeVisible({
      timeout: 60_000,
    });
    await tabs.getByRole("tab", { name: "Finished" }).click();
    await expect(page).toHaveURL(/tab=finished/);
    await expect(page.getByTestId("boson-jobs-data-table")).toBeVisible({ timeout: 60_000 });
    await tabs.getByRole("tab", { name: "Active" }).click();
    await expect(page).not.toHaveURL(/tab=/);
    expect(
      await page.evaluate(() => (window as unknown as { __jobsMark?: boolean }).__jobsMark),
      "switching tabs must not reload the page",
    ).toBe(true);
  });
});

test.describe("pw-boson-jobs-phone", () => {
  test.use({ viewport: { width: 390, height: 844 } });

  test("pw-boson-jobs-phone-width-fits", async ({ page }) => {
    test.setTimeout(90_000);
    await seedAuth(page, "kenji", { yoursFirst: true });
    await page.goto("/boson/jobs", { waitUntil: "domcontentloaded" });
    await waitForHydrated(page);
    const table = page.getByTestId("boson-jobs-data-table");
    await expect(table.locator('[data-testid^="job-card-e2e-yf-media-"]').first()).toBeAttached({
      timeout: 60_000,
    });
    await expect(page.getByTestId("boson-list-caption")).toContainText("Media platform");
    await expect(page.getByTestId("boson-show-select")).toBeVisible();
    // The page itself doesn't scroll sideways; a wide table scrolls inside its card.
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    expect(overflow).toBeLessThanOrEqual(1);
  });
});
