import { defineConfig, devices } from "@playwright/test";

const PORT = process.env.PORT ?? "8080";
const BASE_URL = process.env.BASE_URL ?? `http://127.0.0.1:${PORT}`;
const isCI = !!process.env.CI;

// In CI the workflow boots the binary itself (so we can stream its logs and
// fail fast on a panic). Locally we rely on `./dev.sh` already being up or
// `./e2e.sh` to manage the lifecycle.
export default defineConfig({
  testDir: "./e2e",
  // Every test makes its own accounts (emails derived from the test id), so
  // tests can share the server and database while running side by side.
  fullyParallel: true,
  // Tests mostly wait on the server, so CI can use every core; locally
  // Playwright's default of half leaves room for the editor and dev server.
  workers: isCI ? "100%" : undefined,
  retries: isCI ? 1 : 0,
  reporter: isCI ? [["github"], ["html", { open: "never" }]] : "list",
  timeout: 30_000,
  expect: { timeout: 5_000 },
  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
