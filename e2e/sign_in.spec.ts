import { test, expect, issueSignInCode, testEmail } from "./fixtures";
import type { Page } from "@playwright/test";

// Accounts can be created at `/sign-up` with any email address. They start
// unverified and only get a session once the emailed code is entered, which
// is also how they sign in at `/sign-in` from then on. Mail delivery is
// unreachable in tests, so codes come from the test-auth shim.

async function signUp(page: Page, first: string, last: string, email: string) {
  await page.goto("/sign-up");
  await page.getByLabel("First name").fill(first);
  await page.getByLabel("Last name").fill(last);
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: "Sign Up with Email" }).click();
  await expect(page.locator("#otp_delivery_help")).toBeVisible();
}

async function hasSession(page: Page) {
  return (await page.context().cookies()).some((c) => c.name === "auth_token");
}

test("signing up with email verifies the address, then signs in with a code", async ({
  page,
  request,
}, testInfo) => {
  const email = testEmail(testInfo, "signup");

  // Hand-typed addresses are folded to lowercase.
  await signUp(page, "Ada", "Lovelace", email.toUpperCase());
  await expect(page.locator("#sign_in_form")).toContainText(email);
  await expect(page.getByLabel("Verification code")).toBeVisible();

  // Until the code is entered the account exists but has no session.
  expect(await hasSession(page)).toBe(false);

  // A wrong guess spends one of the five attempts, and says no more than an
  // unknown address would.
  const code = await issueSignInCode(request, email);
  const wrong = code === "000000" ? "111111" : "000000";
  await page.getByLabel("Verification code").fill(wrong);
  await page.getByRole("button", { name: "Verify Email" }).click();
  await expect(page.locator("#sign_in_form [role=alert]")).toContainText(
    "That code is wrong or has expired.",
  );

  await page.getByLabel("Verification code").fill(code);
  await page.getByRole("button", { name: "Verify Email" }).click();
  await page.waitForURL((url) => url.pathname === "/");
  await expect(page.getByText("Signed in as Ada Lovelace")).toBeVisible();
  expect(await hasSession(page)).toBe(true);

  // The session cookie lasts as long as the token (a year), so it survives
  // closing the browser. Without a Max-Age it would be a session cookie,
  // which Playwright reports as expires === -1.
  const auth = (await page.context().cookies()).find(
    (c) => c.name === "auth_token",
  )!;
  const days = (auth.expires - Date.now() / 1000) / 86_400;
  expect(days).toBeGreaterThan(364);
  expect(days).toBeLessThanOrEqual(365);

  // Signed out, the now-verified account signs back in from /sign-in.
  // (Replay and attempt limits are tested against the database in Rust.)
  await page.context().clearCookies();
  await page.goto("/sign-in");
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: "Email Me a Sign-in Code" }).click();
  await page
    .getByLabel("Sign-in code")
    .fill(await issueSignInCode(request, email));
  await page.getByRole("button", { name: "Sign In" }).click();
  await page.waitForURL((url) => url.pathname === "/");
  await expect(page.getByText("Signed in as Ada Lovelace")).toBeVisible();
});

test("signing in doesn't reveal whether an address has an account", async ({
  page,
}, testInfo) => {
  const email = testEmail(testInfo, "nobody");

  await page.goto("/sign-in");
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: "Email Me a Sign-in Code" }).click();
  await expect(page.locator("#sign_in_form")).toContainText(
    `If ${email} has an account, we just emailed it a 6-digit code.`,
  );
  await expect(page.getByLabel("Sign-in code")).toBeVisible();
});

