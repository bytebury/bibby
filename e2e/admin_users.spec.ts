import { test, expect, signInAs } from "./fixtures";
import type { Page } from "@playwright/test";

// Admin user management: the Manage Users list, the per-user admin page
// (notes, flags, sign-ins, linked accounts, edit/delete), and Site Analytics.
//
// The database is shared across specs, so every account is derived from the
// test id. Accounts that are never browsed as keep the sign-in IP the shim
// recorded, which is what the same-IP suggestions match on. Nothing here
// locks an account that shares 127.0.0.1, or later sign-ups from that IP
// would be locked automatically.

const RUN = Date.now().toString(36);

function emails(testId: string) {
  // The run timestamp keeps a retry (or a rerun against the same database)
  // from inheriting the previous attempt's flags, notes, and links.
  const tag = `${testId}-${RUN}`.toLowerCase();
  return {
    admin: `playwright+admin+${tag}@example.com`,
    user: `playwright+user+${tag}@example.com`,
    other: `playwright+other+${tag}@example.com`,
  };
}

/** A per-test private IP, so same-IP matches only see this test's accounts. */
function ipFor(testId: string) {
  const n = [...`${testId}-${RUN}`].reduce(
    (h, c) => (h * 31 + c.charCodeAt(0)) >>> 0,
    7,
  );
  return `10.${(n >> 16) & 255}.${(n >> 8) & 255}.${n & 255}`;
}

async function openUserPage(page: Page, email: string) {
  await page.goto(`/users?q=${encodeURIComponent(email)}`);
  await page
    .locator("table")
    .getByRole("link", { name: "Test User", exact: true })
    .click();
  await expect(page.locator(".user-email")).toHaveText(email);
}

test("admin pages are off limits to everyone else", async ({
  page,
  request,
}, testInfo) => {
  const { user } = emails(testInfo.testId);
  await signInAs(request, page.context(), user, "free");

  for (const path of ["/users", "/admin/analytics"]) {
    const response = await page.goto(path);
    expect(response?.status(), path).toBe(403);
  }
});

test("admins filter the user list by plan and flag", async ({
  page,
  request,
}, testInfo) => {
  const { admin, user } = emails(testInfo.testId);
  await signInAs(request, null, user, "professional");
  await signInAs(request, page.context(), admin, "admin");

  await page.goto("/users");
  await page.getByPlaceholder("Search by email, name, or IP").fill(user);
  await expect(page.locator("#list table")).toContainText(user);

  await page.locator(".filter-chip", { hasText: "Free" }).click();
  await expect(page.locator("#list")).toContainText("No users found.");
  await page.locator(".filter-chip", { hasText: "Professional" }).click();
  await expect(page.locator("#list table")).toContainText(user);
  await expect(page).toHaveURL(/role=professional/);
});

test("admins leave notes, flag accounts, and link duplicates", async ({
  page,
  request,
}, testInfo) => {
  const { admin, user, other } = emails(testInfo.testId);
  const sharedIp = ipFor(testInfo.testId);
  await signInAs(request, null, user, "free", { ip: sharedIp });
  await signInAs(request, null, other, "free", { ip: sharedIp });
  await signInAs(request, page.context(), admin, "admin");

  await openUserPage(page, user);

  // Sign-in history shows the shim's sign-in.
  await expect(page.locator("#sign-ins")).toContainText(sharedIp);

  // Notes are private to admins and can be removed again.
  await page.getByLabel("New note").fill("Refunded on 9/3");
  await page.getByRole("button", { name: "Add note" }).click();
  await expect(page.locator("#notes .note-body")).toHaveText("Refunded on 9/3");
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Delete note" }).click();
  await expect(page.locator("#notes")).toContainText("No notes yet.");

  // Flagging is shown on the page and in the flagged-only list.
  await page.getByLabel("More actions").click();
  await page.getByRole("button", { name: "Flag", exact: true }).click();
  await expect(page.locator(".flagged-badge")).toBeVisible();
  await page.goto(`/users?flagged=on&q=${encodeURIComponent(user)}`);
  await expect(page.locator("#list table .flag-icon")).toBeVisible();

  // The other account shared an IP, so it's suggested and can be linked.
  await openUserPage(page, user);
  const links = page.locator("#links");
  await expect(links).toContainText(other);
  await links
    .locator("li", { hasText: other })
    .getByRole("button", { name: "Link" })
    .click();
  await expect(page.locator("#links")).toContainText("Unlink");
  await page.reload();
  await expect(page.locator(".linked-badge")).toHaveText("Linked to 1 account");

  // Linking by a typo'd address explains itself inline.
  await page.getByLabel("Link another account").fill("nobody@example.com");
  await page
    .locator("#links form")
    .first()
    .getByRole("button", { name: "Link" })
    .click();
  await expect(page.locator("#links [role=alert]")).toHaveText(
    "No account uses that email address.",
  );

  page.once("dialog", (dialog) => dialog.accept());
  await page.locator("#links").getByRole("button", { name: "Unlink" }).click();
  await expect(page.locator("#links")).toContainText(
    "Not linked to any other accounts.",
  );
});

test("admins edit a plan with an end date and delete accounts", async ({
  page,
  request,
}, testInfo) => {
  const { admin, user } = emails(testInfo.testId);
  await signInAs(request, null, user, "free", { ip: ipFor(testInfo.testId) });
  await signInAs(request, page.context(), admin, "admin");

  await openUserPage(page, user);
  await page.getByLabel("More actions").click();
  await page.getByRole("button", { name: "Edit" }).click();
  const modal = page.locator("#modal");
  await modal.getByLabel("Role").selectOption("professional");
  await modal.getByLabel(/Plan end date/).fill("2099-01-31");
  await modal.getByRole("button", { name: "Save" }).click();
  await expect(page.locator("main")).toContainText("January 31, 2099");
  await expect(page.locator("main .badge-success").first()).toHaveText(
    "professional",
  );

  // An address another account already uses is refused with a reason.
  await page.getByLabel("More actions").click();
  await page.getByRole("button", { name: "Edit" }).click();
  await modal.locator("input[name=email]").fill(admin);
  await modal.getByRole("button", { name: "Save" }).click();
  await expect(page.locator("#toast")).toContainText(
    "Another account already uses that email address.",
  );
  await page.reload();

  await page.getByLabel("More actions").click();
  page.once("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Delete user" }).click();
  await page.waitForURL((url) => url.pathname === "/users");
  await page.goto(`/users?q=${encodeURIComponent(user)}`);
  await expect(page.locator("#list")).toContainText("No users found.");
});

test("site analytics summarises the user base", async ({
  page,
  request,
}, testInfo) => {
  const { admin } = emails(testInfo.testId);
  await signInAs(request, page.context(), admin, "admin");

  await page.goto("/admin/analytics");
  await expect(
    page.getByRole("heading", { name: "Site Analytics" }),
  ).toBeVisible();
  // Browsing as the admin counts as activity today.
  const activeUsers = page.locator(".kpi", { hasText: "Active users" });
  await expect(activeUsers.locator(".text-2xl")).not.toHaveText("0");
  await expect(page.locator("#dau-chart")).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Top countries by user count" }),
  ).toBeVisible();
});
