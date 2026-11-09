import {
  expect,
  type APIRequestContext,
  type APIResponse,
  type BrowserContext,
} from "@playwright/test";

export { test, expect } from "@playwright/test";

const TEST_AUTH_TOKEN = process.env.TEST_AUTH_TOKEN ?? "local-e2e-token";

export type Role = "free" | "professional" | "admin";

/**
 * Mints a session for `email` against the test-auth shim and copies the
 * resulting `auth_token` cookie into the browser context. The account is
 * created if needed and always gets `role`. The sign-in is recorded from
 * `ip` (default 127.0.0.1). See `src/infra/api/auth_test.rs`.
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
  const response = await request.post("/auth/test/sign-in", {
    headers: { "X-Test-Auth-Token": TEST_AUTH_TOKEN },
    form,
  });
  expect(response.status(), await response.text()).toBe(204);
  if (context) await copyAuthCookie(response, context);
}

async function copyAuthCookie(response: APIResponse, context: BrowserContext) {
  const header = response
    .headersArray()
    .find(
      (h) =>
        h.name.toLowerCase() === "set-cookie" &&
        h.value.startsWith("auth_token="),
    );
  expect(header, "auth_token cookie").toBeTruthy();
  const value = header!.value.split(";")[0].slice("auth_token=".length);
  const baseURL = new URL(
    process.env.BASE_URL ?? `http://127.0.0.1:${process.env.PORT ?? "8080"}`,
  );
  await context.addCookies([
    {
      name: "auth_token",
      value,
      domain: baseURL.hostname,
      path: "/",
      httpOnly: true,
      sameSite: "Lax",
    },
  ]);
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
