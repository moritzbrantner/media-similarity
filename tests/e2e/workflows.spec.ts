import { expect, test } from "@playwright/test";
import { installDefaultApiMocks } from "./support/api-mocks";

test("edits and persists processing workflows from the UI", async ({ page }) => {
  const mocks = await installDefaultApiMocks(page);

  await page.goto("/");
  await page.getByRole("button", { name: "Open workflow editor" }).click();

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

  await page.getByRole("button", { name: "Index Sources" }).last().click();
  await expect(page.getByText("indexing complete: 3 media item(s)").first()).toBeVisible();
});
