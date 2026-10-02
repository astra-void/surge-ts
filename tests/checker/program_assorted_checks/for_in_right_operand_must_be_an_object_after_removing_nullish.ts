// @surge-compare: messages
enum E { A }
declare let u: number | undefined;
declare let o: object | undefined;
declare let s: string;
declare let un: unknown;
for (const k in null) {}
for (const k in undefined) {}
for (const k in u) {}
for (const k in s) {}
for (const k in un) {}
for (const k in o) {}
for (const k in E) {}
for (const k in [1]) {}
