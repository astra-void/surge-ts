import { boundary, version } from "boundarykit";
import { openPanel } from "boundarykit/panel";

const label: string = boundary("root").label;
const depth: number = boundary("root").depth;
const release: string = version;
const open: boolean = openPanel("main").open;
const badDepth: string = boundary("root").depth;

export { label, depth, release, open, badDepth };
