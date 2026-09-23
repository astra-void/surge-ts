declare function flag(): boolean;
declare function count(): number;
declare const holder: { n: number };

export let literal = ++1;
export let bool = ++true;
export let called = count()++;
export let flagged = flag()--;
export let object = ++{ n: 1 };
export let text = --"text";
export let chained = holder?.n++;
export let member = holder.n++;
