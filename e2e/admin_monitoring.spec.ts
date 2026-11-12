import { test, expect, signInAs } from "./fixtures";

test("admins see the app's health, traffic and recent logs", async ({ page, request, context }, testInfo) => {
  await signInAs(request, context, `playwright+monitoring-admin+${testInfo.testId}@example.com`, "admin");

  await page.goto("/admin/monitoring");
  await expect(page.getByRole("heading", { name: "Monitoring", level: 1 })).toBeVisible();

  // The test sign-in shim is on in this environment, which the page calls out.
  const health = page.locator("#health");
  await expect(health).toHaveClass(/is-critical/);
  await expect(health).toContainText("ENABLE_TEST_AUTH");

  await expect(page.getByRole("heading", { name: "Traffic, last hour" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Configuration" })).toBeVisible();
  await expect(page.getByText("JWT_SECRET")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Largest tables" })).toBeVisible();

  // Startup logs are in the buffer; filtering to errors only narrows them.
  const logs = page.locator("#log-rows");
  await expect(logs.locator(".monitoring-log").first()).toBeVisible();
  await page.getByLabel("Search").fill("listening on");
  await expect(logs).toContainText("listening on http://localhost");
  await page.getByLabel("Search").fill("");
  const refreshed = page.waitForResponse((r) => r.url().includes("/admin/monitoring/logs"));
  await page.getByLabel("Level").selectOption("error");
  await refreshed;
  await expect(logs.locator(".monitoring-log:not(.level-error)")).toHaveCount(0);
});

test("non-admins can't open the monitoring page", async ({ page, request, context }, testInfo) => {
  await signInAs(request, context, `playwright+monitoring-user+${testInfo.testId}@example.com`, "professional");
  for (const path of ["/admin/monitoring", "/admin/monitoring/status", "/admin/monitoring/logs"]) {
    expect((await page.goto(path))?.status()).toBe(403);
  }
});
