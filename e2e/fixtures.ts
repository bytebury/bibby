import {
  expect,
  type APIRequestContext,
  type BrowserContext,
  type TestInfo,
} from "@playwright/test";

export { test, expect } from "@playwright/test";

const TEST_AUTH_TOKEN = process.env.TEST_AUTH_TOKEN ?? "local-e2e-token";

export type Role = "free" | "professional" | "admin";

// Each worker loads this module, and a retry runs in a fresh worker, so the
// suffix keeps a retry (or a rerun against the same database) from inheriting
// the previous attempt's accounts, flags, and notes.
const RUN = Date.now().toString(36);

/** A per-test, per-run address, e.g. `testEmail(testInfo, "admin")`. */
export function testEmail(testInfo: TestInfo, who: string) {
  return `playwright+${who}+${testInfo.testId}-${RUN}@example.com`.toLowerCase();
}

/**
 * Mints a session for `email` against the test-auth shim. With a browser
 * context the request goes through `context.request`, which shares the
 * context's cookie jar, so its pages are signed in; with `null` the account is
 * only created. The account always gets `role`, and the sign-in is recorded
 * from `ip` (default 127.0.0.1). See `src/infra/api/auth_test.rs`.
 */
export async function signInAs(
  request: APIRequestContext,
  context: BrowserContext | null,
  email: string,
  role: Role = "free",
  profile: { fullName?: string; ip?: string } = {},
) {
  const form: Record<string, string> = { email, role };
  if (profile.fullName !== undefined) form.full_name = profile.fullName;
  if (profile.ip !== undefined) form.ip = profile.ip;
  const response = await (context?.request ?? request).post("/auth/test/sign-in", {
    headers: { "X-Test-Auth-Token": TEST_AUTH_TOKEN },
    form,
  });
  expect(response.status(), await response.text()).toBe(204);
}

/**
 * Issues a fresh emailed sign-in code for `email` and returns the plaintext
 * (test-auth shim only — the real code only ever lives in the email). As the
 * newest code for the address it supersedes any code the UI requested. See
 * `src/infra/api/auth_test.rs`.
 */
export async function issueSignInCode(
  request: APIRequestContext,
  email: string,
): Promise<string> {
  const response = await request.post("/auth/test/sign-in-code", {
    headers: { "X-Test-Auth-Token": TEST_AUTH_TOKEN },
    form: { email },
  });
  expect(response.status(), await response.text()).toBe(200);
  return (await response.text()).trim();
}
