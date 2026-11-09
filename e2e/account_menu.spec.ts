import { test, expect, signInAs } from "./fixtures";
import type { Page } from "@playwright/test";

// The avatar menu in the navbar and the settings page it links to.

const RUN = Date.now().toString(36);

function emailFor(testId: string, who: string) {
  return `playwright+${who}+${testId}-${RUN}@example.com`.toLowerCase();
}

function accountMenu(page: Page) {
  return page.locator("details.account-menu");
}

async function openAccountMenu(page: Page) {
  await page.getByLabel("Account menu").click();
  return accountMenu(page).locator(".menu-items");
}

test("signed-out visitors get a sign-in button instead of the avatar", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByLabel("Account menu")).toHaveCount(0);
  await expect(
    page.locator("nav").getByRole("link", { name: "Sign in", exact: true }),
  ).toBeVisible();
});

test("the avatar menu offers settings and sign out, but no admin links", async ({
  page,
  request,
}, testInfo) => {
  const email = emailFor(testInfo.testId, "free");
  await signInAs(request, page.context(), email, "free");
  await page.goto("/");

  const menu = await openAccountMenu(page);
  await expect(menu).toContainText(email);
  await expect(menu.getByRole("link", { name: "Settings" })).toBeVisible();
  await expect(menu.getByRole("button", { name: "Sign out" })).toBeVisible();
  await expect(menu.getByRole("link", { name: "Users" })).toHaveCount(0);
  await expect(menu.getByRole("link", { name: "Analytics" })).toHaveCount(0);

  await menu.getByRole("button", { name: "Sign out" }).click();
  await expect(page.getByLabel("Account menu")).toHaveCount(0);
});

test("admins find the admin links in the avatar menu", async ({
  page,
  request,
}, testInfo) => {
  await signInAs(request, page.context(), emailFor(testInfo.testId, "admin"), "admin");
  await page.goto("/");

  // Admin links live only in the menu, not the navbar.
  await expect(
    page.locator("nav .navbar-links > a", { hasText: "Users" }),
  ).toHaveCount(0);

  const menu = await openAccountMenu(page);
  for (const name of ["Users", "Analytics", "Countries", "Announcements", "Settings"]) {
    await expect(menu.getByRole("link", { name })).toBeVisible();
  }
  await menu.getByRole("link", { name: "Analytics" }).click();
  await expect(page).toHaveURL(/\/admin\/analytics$/);
});

test("users rename themselves from the settings page", async ({
  page,
  request,
}, testInfo) => {
  await signInAs(request, page.context(), emailFor(testInfo.testId, "free"), "free");
  await page.goto("/");

  const menu = await openAccountMenu(page);
  await menu.getByRole("link", { name: "Settings" }).click();
  await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  await expect(page.locator(".plan-name")).toHaveText("Free");

  await page.getByLabel("First name").fill("  Grace ");
  await page.getByLabel("Last name").fill("Hopper");
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByLabel("First name")).toHaveValue("Grace");
  await openAccountMenu(page);
  await expect(accountMenu(page)).toContainText("Grace Hopper");
});

test("settings require a session", async ({ page }) => {
  const response = await page.goto("/settings");
  expect(response?.status()).toBe(401);
});
