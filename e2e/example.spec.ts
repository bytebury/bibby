import { test, expect } from "@playwright/test";

test("homepage renders with the Bibby title", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/Bibby/i);
});
