const text: string = "ok";
const count: number = 1;
const flag: boolean = true;
const opaque: unknown = text;
const loose: any = count;
let nothing: void = undefined;

const widened: string = `${text}-${count}`;
const sum: number = count + 1;

export { text, count, flag, opaque, loose, nothing, widened, sum };
