import AxeBuilder from "@axe-core/playwright";
import { test, expect, signInAs, testEmail } from "./fixtures";
import type { Page } from "@playwright/test";

// Keyboard and screen-reader behaviour, plus an automated WCAG 2.1 AA scan of
// the main pages. The scan catches markup mistakes (missing names, contrast,
// landmarks); the other tests cover behaviour a scanner can't see.

async function expectNoViolations(page: Page) {
  const { violations } = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "best-practice"])
    .analyze();
  expect(
    violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(", ")}`),
  ).toEqual([]);
}

async function openNewAnnouncement(page: Page) {
  await page.goto("/announcements");
  const opener = page.getByRole("button", { name: "New announcement" });
  await opener.click();
  const dialog = page.getByRole("dialog", { name: "New announcement" });
  await expect(dialog).toBeVisible();
  // Let the fade-in finish: mid-animation text is translucent, which the
  // contrast check would rightly flag.
  await page.evaluate(() =>
    Promise.all(document.getAnimations().map((animation) => animation.finished)),
  );
  return opener;
}

test("public pages pass an accessibility scan", async ({ page }) => {
  for (const path of ["/", "/sign-in", "/sign-up", "/blogs", "/privacy", "/terms"]) {
    await page.goto(path);
    await expectNoViolations(page);
  }
});

test("signed-in pages, an open menu and an open modal pass an accessibility scan", async ({
  page,
}, testInfo) => {
  const admin = testEmail(testInfo, "admin");
  await signInAs(page.request, page.context(), admin, "admin");
  for (const path of [
    "/",
    "/settings",
    "/users",
    "/announcements",
    "/countries",
    "/admin/analytics",
    "/admin/monitoring",
    "/admin/feature-toggles",
  ]) {
    await page.goto(path);
    await expectNoViolations(page);
  }
  // An account page: activity strip, sign-ins, notes.
  await page.goto(`/users?q=${encodeURIComponent(admin)}`);
  await page.locator('#list a[href^="/users/"]:visible').first().click();
  await expect(page.locator(".user-email")).toHaveText(admin);
  await expectNoViolations(page);
  await page.getByLabel("Account menu").click();
  await expectNoViolations(page);

  await openNewAnnouncement(page);
  await expectNoViolations(page);
});

test("modals are keyboard dialogs: focus moves in, stays in, and returns on close", async ({
  page,
}, testInfo) => {
  await signInAs(page.request, page.context(), testEmail(testInfo, "admin"), "admin");
  await page.goto("/announcements");
  // A stray Escape used to make the next modal close itself as it opened.
  await page.keyboard.press("Escape");
  const opener = await openNewAnnouncement(page);
  const dialog = page.getByRole("dialog", { name: "New announcement" });

  // Focus starts on the first field, and Tab never leaves the dialog.
  await expect(dialog.getByLabel("Title")).toBeFocused();
  for (const key of [...Array(12).fill("Tab"), ...Array(12).fill("Shift+Tab")]) {
    await page.keyboard.press(key);
    expect(await dialog.evaluate((d) => d.contains(document.activeElement)), key).toBe(true);
  }

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(opener).toBeFocused();

  await openNewAnnouncement(page);
  await page.getByRole("button", { name: "Close" }).click();
  await expect(dialog).toBeHidden();
});

test("the skip link and the account menu work from the keyboard", async ({ page }, testInfo) => {
  await signInAs(page.request, page.context(), testEmail(testInfo, "free"), "free");
  await page.goto("/");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("link", { name: "Skip to content" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("main")).toBeFocused();

  // Escape closes the menu and puts focus back on its button.
  const toggle = page.getByLabel("Account menu");
  await toggle.click();
  const settings = page.locator("details.account-menu").getByRole("link", { name: "Settings" });
  await settings.focus();
  await page.keyboard.press("Escape");
  await expect(settings).toBeHidden();
  await expect(toggle).toBeFocused();
});

