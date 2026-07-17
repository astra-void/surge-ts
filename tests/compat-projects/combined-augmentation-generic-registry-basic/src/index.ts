import { createRegistry, defaults, extras, Registry } from "configlib";

const registry: Registry<string> = createRegistry<string>();
registry.set("greeting", "hello");
const has: boolean = registry.has("greeting");
const stored: string | undefined = registry.get("greeting");
const timeout: number = extras.timeout;
const retries: number = defaults.retries;
const bad: number = registry.get("greeting");

export { has, stored, timeout, retries, bad };
