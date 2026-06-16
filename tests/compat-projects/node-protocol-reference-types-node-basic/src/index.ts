/// <reference types="node" />
import { readFile } from "node:fs";

readFile("config.json");
readFile(42);

export {};
