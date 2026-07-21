import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";

// Pure-function tests only — no DOM, no Svelte compilation, so the SvelteKit plugin isn't needed.
// `$lib` still has to resolve the way the app resolves it.
export default defineConfig({
  resolve: {
    alias: { $lib: fileURLToPath(new URL("./src/lib", import.meta.url)) },
  },
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
