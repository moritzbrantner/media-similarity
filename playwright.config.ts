import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "tests/e2e",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? [["dot"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: "http://127.0.0.1:5179",
    trace: "on-first-retry",
  },
  webServer: {
    command: "bunx vite --host 127.0.0.1 --port 5179 --strictPort",
    reuseExistingServer: false,
    url: "http://127.0.0.1:5179",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
