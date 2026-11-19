import { test, expect, signInAs, issueSignInCode } from "./fixtures";

// Account security: who becomes an admin, ending sessions, cross-site
// requests, untrusted names in admin pages, and running without Stripe.

const RUN = Date.now().toString(36);

function emailFor(testId: string, who: string) {
  return `playwright+${who}+${testId}-${RUN}@example.com`.toLowerCase();
}

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
  const email = emailFor(testInfo.testId, "unlisted");
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
  const email = emailFor(testInfo.testId, "everywhere");
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

test("state-changing requests from another site are refused", async ({
  page,
  request,
}, testInfo) => {
  await signInAs(request, page.context(), emailFor(testInfo.testId, "csrf"), "free");
  const response = await page.request.patch("/settings", {
    headers: { Origin: "https://evil.example" },
    form: { first_name: "Mallory", last_name: "Evil" },
  });
  expect(response.status()).toBe(403);
});

test("a user's name can't run script on the admin user page", async ({
  page,
  request,
}, testInfo) => {
  const victim = emailFor(testInfo.testId, "named");
  await signInAs(request, null, victim, "free", {
    fullName: "</script><script>window.__pwned = true</script>",
  });
  await signInAs(request, page.context(), emailFor(testInfo.testId, "admin"), "admin");

  await page.goto(`/users?q=${encodeURIComponent(victim)}`);
  await page.locator('#list a[href^="/users/"]:visible').first().click();
  await expect(page.locator(".user-email")).toHaveText(victim);
  expect(await page.evaluate(() => (window as { __pwned?: boolean }).__pwned)).toBeUndefined();
});

test("without Stripe there is no billing to reach", async ({ page, request }, testInfo) => {
  await signInAs(request, page.context(), emailFor(testInfo.testId, "free"), "free");
  await page.goto("/");
  await expect(page.locator("nav").getByRole("link", { name: "Upgrade" })).toHaveCount(0);
  expect((await page.goto("/upgrade"))?.status()).toBe(404);
});
