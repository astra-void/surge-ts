declare const o1: any;
declare const q1: { b: string } | undefined;
for (q1?.b in o1) {}
export {};
