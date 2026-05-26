import { defineConfig, devices } from "@playwright/test";

const PORT = process.env.PORT ?? "8080";
const BASE_URL = process.env.BASE_URL ?? `http://127.0.0.1:${PORT}`;
const isCI = !!process.env.CI;

// In CI the workflow boots the binary itself (so we can stream its logs and
// fail fast on a panic). Locally we rely on `./dev.sh` already being up or
// `./e2e.sh` to manage the lifecycle.
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  retries: isCI ? 1 : 0,
  reporter: isCI ? [["github"], ["html", { open: "never" }]] : "list",
  timeout: 30_000,
  expect: { timeout: 5_000 },
  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
