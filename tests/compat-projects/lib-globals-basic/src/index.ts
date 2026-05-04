console.log();
console.log("ok");
console.log("a", "b");
console.warn("warn", 1);
console.error("error", 1, true);

const num1: number = Math.max(1, 2);
const num2: string = Math.max(1, 2); // TS2322
Math.max(1);
Math.max(1, 2, 3);
Math.min(1, 2, 3);

const obj = JSON.parse("{}");

const promise: Promise<string> = null as any;

const arr: Array<string> = ["a"];
const arrInvalid: Array<string> = [1]; // TS2322

const arr2: ReadonlyArray<number> = [1];
const arr2Invalid: ReadonlyArray<number> = ["a"]; // TS2322

const r: Record<string, any> = null as any;
const p: Partial<any> = null as any;
const pk: Pick<any, any> = null as any;
const o: Omit<any, any> = null as any;
const rt: ReturnType<any> = null as any;
const pa: Parameters<any> = null as any;
