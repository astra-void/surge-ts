import { defineConfig } from "vitest/config";

// The native binding is a Node addon: tests run in forked processes, and a
// program loads the whole standard library, so tests get time for that.
export default defineConfig({
	test: {
		include: ["test/**/*.test.mjs"],
		pool: "forks",
		testTimeout: 60_000,
	},
});
