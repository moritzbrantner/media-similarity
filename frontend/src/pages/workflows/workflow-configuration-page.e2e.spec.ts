import { expect, test } from "@playwright/test";

import { mockEndpointFailure } from "../../../../tests/e2e/support/api-mocks";
import { installDefaultApiMocks } from "../../../../tests/e2e/support/api-mocks";

test("edits, validates, saves, and resets processing workflows", async ({ page }) => {
  const mocks = await installDefaultApiMocks(page);

  await page.goto("/workflows");

  await expect(page.getByRole("heading", { name: "Processing Workflows" })).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Workflow document" })).toHaveValue(
    "static_image",
  );
  await expect(page.getByRole("button", { exact: true, name: "Decode image" })).toBeVisible();
  await expect(page.getByText("No workflow diagnostics.")).toBeVisible();

  await page.getByRole("button", { name: "Validate" }).click();
  await expect.poll(() => mocks.workflowValidations.length).toBe(1);

  await page.getByRole("button", { name: "Save" }).click();
  await expect.poll(() => mocks.workflowPuts.length).toBe(1);
  await expect(page.getByText("Saved workflows.")).toBeVisible();

  await page.getByRole("button", { name: "Reset" }).click();
  await expect.poll(() => mocks.workflowResets.length).toBe(1);
});

test("renders workflow save and validation failures", async ({ page }) => {
  await installDefaultApiMocks(page);
  await mockEndpointFailure(page, "**/api/workflows/validate", 500, "validation failed");
  await page.goto("/workflows");

  await page.getByRole("button", { name: "Validate" }).click();
  await expect(page.getByText("validation failed")).toBeVisible();

  await page.unroute("**/api/workflows");
  await page.route("**/api/workflows", async (route) => {
    if (route.request().method() === "PUT") {
      await route.fulfill({
        json: { detail: "workflow save failed" },
        status: 500,
      });
      return;
    }

    await route.fallback();
  });
  await page.getByRole("button", { name: "Save" }).click();

  await expect(page.getByText("workflow save failed")).toBeVisible();
  await expect(page.getByText("Saved workflows.")).toHaveCount(0);
});
