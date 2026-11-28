import type { Locator, Page } from "@playwright/test";
import { test, expect, seedAuth, waitForHydrated } from "./fixtures";

// Kenji's Media platform team owns e2e_media_transcode and the e2e-media queue; Ada's Billing
// engineering owns e2e_invoice_sync. The seed adds 25 queued jobs and 25 runs for each, so
// "yours first" puts a different 20 on page 1 for each of them.

async function open(page: Page, path: string) {
  await page.goto(path, { waitUntil: "domcontentloaded" });
  await waitForHydrated(page);
}

/** The Owner value on one Tasks list card. */
function ownerValue(table: Locator, name: string): Locator {
  return table
    .getByTestId(`data-table-list-card-${name}`)
    .locator(".orbital-data-table__list-card-field", { hasText: "Owner" })
    .locator(".orbital-data-table__list-card-value");
}

/** Ids from the `<prefix><id>` test ids inside `scope`, in page order. */
async function idsByPrefix(scope: Locator, prefix: string): Promise<string[]> {
  return scope
    .locator(`[data-testid^="${prefix}"]:not(style)`)
    .evaluateAll(
      (els, p) => els.map((el) => (el.getAttribute("data-testid") ?? "").slice(p.length)),
      prefix,
    );
}

test.describe("pw-boson-yours-first", () => {
  test("pw-boson-yours-first-tasks-differ-by-viewer", async ({ page }) => {
    test.setTimeout(120_000);
    const kenji = await seedAuth(page, "kenji", { yoursFirst: true });
    const { media_task: media, invoice_task: invoice } = kenji.fixtures;

    await open(page, "/boson/tasks");
    const caption = page.getByTestId("boson-list-caption");
    await expect(caption).toHaveText(
      "Yours first: task types owned by Media platform, then everyone else's.",
      { timeout: 60_000 },
    );
    const table = page.getByTestId("boson-tasks-data-table");
    await expect(table.getByTestId(`data-table-list-card-${media}`)).toBeVisible({
      timeout: 60_000,
    });
    const kenjiOrder = await idsByPrefix(table, "data-table-list-card-");
    expect(kenjiOrder[0]).toBe(media);
    // List cards show plain text, so the owner reads "… · Yours" there.
    await expect(ownerValue(table, media)).toHaveText("Media platform · Yours");

    await seedAuth(page, "ada");
    await open(page, "/boson/tasks");
    await expect(caption).toHaveText(
      "Yours first: task types owned by Billing engineering, then everyone else's.",
      { timeout: 60_000 },
    );
    await expect(table.getByTestId(`data-table-list-card-${invoice}`)).toBeVisible({
      timeout: 60_000,
    });
    const adaOrder = await idsByPrefix(table, "data-table-list-card-");
    expect(adaOrder[0]).toBe(invoice);
    expect(adaOrder).not.toEqual(kenjiOrder);
  });

  test("pw-boson-yours-first-jobs-keep-order-across-pages", async ({ page }) => {
    test.setTimeout(120_000);
    await seedAuth(page, "kenji", { yoursFirst: true });
    await open(page, "/boson/jobs");
    const table = page.getByTestId("boson-jobs-data-table");
    await expect(table.locator('[data-testid^="job-card-e2e-yf-media-"]').first()).toBeVisible({
      timeout: 60_000,
    });
    const page1 = await idsByPrefix(table, "job-card-");
    expect(page1).toHaveLength(20);
    expect(page1.every((id) => id.startsWith("e2e-yf-media-"))).toBe(true);
    expect(page1[0]).toBe("e2e-yf-media-00");

    await table
      .getByTestId("data-table-pagination")
      .getByRole("button", { name: "Next page" })
      .click();
    await expect(table.getByTestId("data-table-footer")).toContainText("21–40", {
      timeout: 60_000,
    });
    await expect.poll(async () => (await idsByPrefix(table, "job-card-"))[0]).toBe(
      "e2e-yf-media-20",
    );
    const page2 = await idsByPrefix(table, "job-card-");
    expect(page2.slice(0, 5).every((id) => id.startsWith("e2e-yf-media-"))).toBe(true);
    expect(page2.slice(5).some((id) => id.startsWith("e2e-yf-media-"))).toBe(false);

    // Ada sees her team's jobs first instead.
    await seedAuth(page, "ada");
    await open(page, "/boson/jobs");
    await expect(table.locator('[data-testid^="job-card-e2e-yf-inv-"]').first()).toBeVisible({
      timeout: 60_000,
    });
    expect((await idsByPrefix(table, "job-card-"))[0]).toBe("e2e-yf-inv-00");
  });

  test("pw-boson-yours-first-only-yours-hides-other-teams", async ({ page }) => {
    test.setTimeout(120_000);
    await seedAuth(page, "ada", { yoursFirst: true });
    await open(page, "/boson/runs?show=mine");
    const caption = page.getByTestId("boson-list-caption");
    await expect(caption).toHaveText(
      "Only yours: task types owned by Billing engineering.",
      { timeout: 60_000 },
    );
    const table = page.getByTestId("boson-runs-data-table");
    await expect(table.locator('[data-testid^="runs-row-e2e-yf-run-inv-"]').first()).toBeVisible({
      timeout: 60_000,
    });
    const runs = await idsByPrefix(table, "runs-row-");
    expect(runs.length).toBeGreaterThan(0);
    expect(runs.every((id) => id.startsWith("e2e-yf-run-inv-"))).toBe(true);
    await expect(table.getByTestId("runs-total")).toContainText("25 runs");

    // Everyone's puts the other teams' runs back.
    await page.getByTestId("boson-show-select").locator("select").selectOption("all");
    await expect(page).toHaveURL(/show=all/);
    await expect(table.locator('[data-testid^="runs-row-e2e-yf-run-media-"]').first()).toBeVisible({
      timeout: 60_000,
    });
  });

  test("pw-boson-yours-first-queues-and-no-team", async ({ page }) => {
    test.setTimeout(120_000);
    const seeded = await seedAuth(page, "kenji", { yoursFirst: true });
    await open(page, "/boson/queues");
    await expect(page.getByTestId("boson-list-caption")).toHaveText(
      "Yours first: queues owned by Media platform, then everyone else's.",
      { timeout: 60_000 },
    );
    const table = page.getByTestId("boson-queues-table");
    await expect(table.getByTestId(`queue-row-${seeded.fixtures.media_pool}`)).toBeVisible({
      timeout: 60_000,
    });
    expect((await idsByPrefix(table, "queue-row-"))[0]).toBe(seeded.fixtures.media_pool);

    // The admins' default ownership doesn't count.
    await seedAuth(page, "admin");
    await open(page, "/boson/queues");
    await expect(page.getByTestId("boson-list-caption")).toHaveText(
      "Your teams own no queues here, so this shows everyone's.",
      { timeout: 60_000 },
    );
  });
});
