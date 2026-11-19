import { test, expect, signInAs, testEmail } from "./fixtures";

// Account security in the browser: ending sessions on other devices and
// running without Stripe. Admin promotion and sign-in codes are tested
// against the database in Rust (`#[sqlx::test]`).

test("signing out everywhere ends sessions on other devices", async ({
  browser,
  request,
}, testInfo) => {
  const email = testEmail(testInfo, "everywhere");
  const laptop = await browser.newContext();
  const phone = await browser.newContext();
  await signInAs(request, laptop, email, "free");
  await phone.addCookies(await laptop.cookies());

  const laptopPage = await laptop.newPage();
  await laptopPage.goto("/settings");
  await laptopPage.getByRole("button", { name: "Sign out everywhere" }).click();
  await laptopPage.waitForURL((url) => url.pathname === "/");

  const phonePage = await phone.newPage();
  expect((await phonePage.goto("/settings"))?.status()).toBe(401);

  await laptop.close();
  await phone.close();
});

test("without Stripe there is no billing to reach", async ({ page, request }, testInfo) => {
  await signInAs(request, page.context(), testEmail(testInfo, "free"), "free");
  await page.goto("/");
  await expect(page.locator("nav").getByRole("link", { name: "Upgrade" })).toHaveCount(0);
  expect((await page.goto("/upgrade"))?.status()).toBe(404);
});
