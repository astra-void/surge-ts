import alias from "./alias";
import { default as named } from "./alias";
import both, { default as bothNamed } from "./alias";
import * as reexported from "./reexport";
import record from "./record";
import Plain from "./plain";
import settings from "./decl";
import marked from "./marked";
import declDefault from "./declDefault";

alias.default();
named.default();
both.default();
bothNamed.version;
reexported.default.default();
reexported.renamed.version;
alias();

record.greeting.toUpperCase();
record.toFixed(1);

new Plain().value;

settings.mode.toUpperCase();
settings.default.valueOf();

marked;
declDefault().toUpperCase();
