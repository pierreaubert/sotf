import { defineConfig } from "vitest/config";
import path from "path";

export default defineConfig({
  test: {
    environment: "jsdom",
    setupFiles: ["./sotf-ui-frontend/tests/test-setup.ts"],
    globals: true,
    include: [
      "./sotf-ui-frontend/tests/**/*.{test,spec}.{js,ts}",
    ],
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      exclude: [
        "node_modules/",
        "./sotf-ui-frontend/tests/test-setup.ts",
        "./sotf-ui-frontend/**/*.d.ts",
        "./sotf-ui-frontend/**/*.config.*",
        "./dist/"
      ],
    },
  },
  resolve: {
    alias: {
      "@": "/sotf-ui-frontend",
      "@audio-player": path.resolve(__dirname, "./sotf-ui-frontend/modules/audio-player"),
      "@audio-capture": path.resolve(__dirname, "./sotf-ui-frontend/modules/audio-capture"),
      "@ui": path.resolve(__dirname, "./sotf-ui-frontend"),
    },
  },
});
