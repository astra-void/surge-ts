export {};

var moduleSelf: any = (moduleSelf = 2);
export function writesLater() { moduleCounter = 1; }
var moduleCounter = 0;

var moduleTyped: string = (moduleTyped = 1, "");
export function writesMissing() { missing = 1; }
