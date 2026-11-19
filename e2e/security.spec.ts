import { test, expect, signInAs, issueSignInCode, testEmail } from "./fixtures";

// Account security: who becomes an admin, ending sessions, cross-site
// requests, untrusted names in admin pages, and running without Stripe.

// Listed in ADMIN_EMAILS by e2e.sh and the Playwright workflow.
const LISTED_ADMIN = "playwright+listed-admin@example.com";

test("an address in ADMIN_EMAILS becomes an admin once it enters its code", async ({
  page,
  request,
}) => {
  await page.goto("/sign-up");
  await page.getByLabel("First name").fill("Listed");
  await page.getByLabel("Last name").fill("Admin");
  await page.getByLabel("Email").fill(LISTED_ADMIN);
  await page.getByRole("button", { name: "Sign Up with Email" }).click();
  await page
    .getByLabel(/Verification code|Sign-in code/)
    .fill(await issueSignInCode(request, LISTED_ADMIN));
  await page.getByRole("button", { name: /Verify Email|Sign In/ }).click();
  await page.waitForURL((url) => url.pathname === "/");

  await page.getByLabel("Account menu").click();
  await expect(
    page.locator("details.account-menu").getByRole("link", { name: "Users" }),
  ).toBeVisible();
});

test("an address not in ADMIN_EMAILS stays a free account", async ({
  page,
  request,
}, testInfo) => {
  const email = testEmail(testInfo, "unlisted");
  await page.goto("/sign-up");
  await page.getByLabel("First name").fill("Not");
  await page.getByLabel("Last name").fill("Listed");
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: "Sign Up with Email" }).click();
  await page.getByLabel("Verification code").fill(await issueSignInCode(request, email));
  await page.getByRole("button", { name: "Verify Email" }).click();
  await page.waitForURL((url) => url.pathname === "/");

  expect((await page.goto("/users"))?.status()).toBe(403);
});

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
