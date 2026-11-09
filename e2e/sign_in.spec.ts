import { test, expect, issueSignInCode } from "./fixtures";
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

test("the homepage links to sign-up and sign-in", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Get started" }).click();
  await expect(page).toHaveURL(/\/sign-up$/);
  await expect(
    page.getByRole("link", { name: "Sign up with Google" }),
  ).toHaveAttribute("href", "/auth/google");

  await page.goto("/");
  await page
    .locator("main")
    .getByRole("link", { name: "Sign in", exact: true })
    .click();
  await expect(page).toHaveURL(/\/sign-in$/);
  await expect(
    page.getByRole("button", { name: "Email Me a Sign-in Code" }),
  ).toBeVisible();
});

test("signing up with email verifies the address, then signs in with a code", async ({
  page,
  request,
}, testInfo) => {
  const email =
    `playwright+signup+${testInfo.testId}@example.com`.toLowerCase();

  // Hand-typed addresses are folded to lowercase.
  await signUp(page, "Ada", "Lovelace", email.toUpperCase());
  await expect(page.locator("#sign_in_form")).toContainText(email);
  await expect(page.getByLabel("Verification code")).toBeVisible();

  // Until the code is entered the account exists but has no session.
  expect(await hasSession(page)).toBe(false);

  // A wrong guess spends one of the five attempts.
  const code = await issueSignInCode(request, email);
  const wrong = code === "000000" ? "111111" : "000000";
  await page.getByLabel("Verification code").fill(wrong);
  await page.getByRole("button", { name: "Verify Email" }).click();
  await expect(page.locator("#sign_in_form [role=alert]")).toContainText(
    "You have 4 tries left.",
  );

  await page.getByLabel("Verification code").fill(code);
  await page.getByRole("button", { name: "Verify Email" }).click();
  await page.waitForURL((url) => url.pathname === "/");
  await expect(page.getByText("Signed in as Ada Lovelace")).toBeVisible();
  expect(await hasSession(page)).toBe(true);

  // A used code can't be replayed.
  await page.context().clearCookies();
  await page.goto("/sign-in");
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: "Email Me a Sign-in Code" }).click();
  await page.getByLabel("Sign-in code").fill(code);
  await page.getByRole("button", { name: "Sign In" }).click();
  await expect(page.locator("#sign_in_form [role=alert]")).toContainText(
    "expired",
  );

  // A fresh code signs the now-verified account back in.
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
  const email =
    `playwright+nobody+${testInfo.testId}@example.com`.toLowerCase();

  await page.goto("/sign-in");
  await page.getByLabel("Email").fill(email);
  await page.getByRole("button", { name: "Email Me a Sign-in Code" }).click();
  await expect(page.locator("#sign_in_form")).toContainText(
    `If ${email} has an account, we just emailed it a 6-digit code.`,
  );
  await expect(page.getByLabel("Sign-in code")).toBeVisible();
});

test("the sign-up form rejects a malformed email", async ({ page }) => {
  await page.goto("/sign-up");
  await page.getByLabel("First name").fill("Ada");
  await page.getByLabel("Last name").fill("Lovelace");
  // `type="email"` would stop the browser from submitting this.
  await page
    .getByLabel("Email")
    .evaluate((el) => el.setAttribute("type", "text"));
  await page.getByLabel("Email").fill("not-an-email");
  await page.getByRole("button", { name: "Sign Up with Email" }).click();
  await expect(page.locator("#sign_in_form [role=alert]")).toHaveText(
    "Enter a valid email address.",
  );
  await expect(page.getByLabel("First name")).toHaveValue("Ada");
});
