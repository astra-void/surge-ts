declare const o: any;
for (const [a] in o) {}
for (var [b] in o) {}
for (const { length } in o) {}
let c: string;
for ([c] in o) {}
for (const k in o) {}
export {};
