import { runtime } from "nodekit";
import { readText, exists } from "nodekit/fs";
import { openStream } from "nodekit/stream";

const platform: string = runtime().platform;
const text: string = readText("a.txt");
const present: boolean = exists("a.txt");
const stream = openStream("main");
const isEsm: true = stream.esm;
const badPid: string = runtime().pid;

export { platform, text, present, isEsm, badPid };
