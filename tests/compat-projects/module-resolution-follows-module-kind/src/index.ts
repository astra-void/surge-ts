import { fromNode } from "inner/node-only";
import { fromBrowser } from "inner/browser-only";
import { fromCjs, fromEsm } from "inner";

export const values = [fromNode, fromBrowser, fromCjs, fromEsm];
