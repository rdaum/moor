import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
    plugins: [react()],
    test: {
        environment: "jsdom",
        globals: true,
        include: ["web-client/**/*.test.{ts,tsx}"],
        setupFiles: ["./vitest.setup.ts"],
    },
});
