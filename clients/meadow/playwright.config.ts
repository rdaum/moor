import { defineConfig } from "@playwright/test";

export default defineConfig({
    testDir: "./e2e",
    testMatch: "**/*.spec.ts",
    fullyParallel: false,
    workers: 1,
    timeout: 90_000,
    expect: { timeout: 15_000 },
    use: { actionTimeout: 15_000, headless: true, trace: "retain-on-failure" },
    outputDir: "test-results",
});
