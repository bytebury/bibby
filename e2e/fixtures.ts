import { expect, type APIRequestContext } from "@playwright/test";

export { test, expect } from "@playwright/test";

const TEST_AUTH_TOKEN = process.env.TEST_AUTH_TOKEN ?? "local-e2e-token";

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
