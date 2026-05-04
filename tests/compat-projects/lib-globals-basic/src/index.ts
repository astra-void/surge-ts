console.log("ok");
console.warn("warn");
console.error("error");

const num1: number = Math.max(1, 2);
const num2: string = Math.max(1, 2);

const obj = JSON.parse("{}");

const promise: Promise<string> = null as any;

const arr: Array<string> = null as any;
const arr2: ReadonlyArray<string> = null as any;

const r: Record<string, any> = null as any;
const p: Partial<any> = null as any;
const pk: Pick<any, any> = null as any;
const o: Omit<any, any> = null as any;
const rt: ReturnType<any> = null as any;
const pa: Parameters<any> = null as any;
