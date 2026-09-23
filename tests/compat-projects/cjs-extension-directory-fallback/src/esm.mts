import { fromDirectory } from "./lib.json";
import { appended } from "./util.js";
import { fromTsDirectory } from "./dir.ts";
import required = require("./lib.json");

export const values = [fromDirectory, appended, fromTsDirectory, required.fromDirectory];
