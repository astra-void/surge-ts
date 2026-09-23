import * as plain from "./vendor/plain";
import dir from "./vendor/dir";
import { x } from "./vendor/explicit.js";
import pkg from "untyped-pkg";
import sub from "untyped-pkg/lib/sub";
import scoped from "@untyped/scoped";
import view from "./vendor/view";
import typed from "./vendor/typed";
import missing from "./vendor/missing";
import "./vendor/plain";

export const all = [plain, dir, x, pkg, sub, scoped, view, typed, missing];
