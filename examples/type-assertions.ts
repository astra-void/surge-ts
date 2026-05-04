const a: number = 1 as any;
const b: string = 1 as number;
const c: string = [] as Array<string>; // Error
const d: boolean = "hello" as any; // No error