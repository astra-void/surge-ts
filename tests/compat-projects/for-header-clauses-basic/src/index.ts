interface Link {
  next: Link | undefined;
  value: number;
}
declare const shape: { present: number };

let moduleCount: number = 0;
let moduleText: string = "";
for (moduleCount = "a"; moduleCount < 3; moduleText = 1) {}

export function clauses(subject: string | number, count: number) {
  for (subject = undefined; count < 3; subject = true) {
    count++;
  }
  for (let index: string = 1; count < 3; index = 2) {
    count++;
  }
  for (count = "s", subject = null; shape.missing; count += shape.absent) {}
  for (; ; count = "t") {
    break;
  }
  return subject;
}

export function headerScope(limit: number) {
  let total = 0;
  for (const step = 0; step < limit; total = step) {
    const step = false;
    if (step) {
      break;
    }
  }
  return total;
}

export function walk(head: Link | undefined, skip: boolean): number {
  let total = 0;
  for (let link = head; link; link = link.next) {
    if (skip) {
      continue;
    }
    total += link.value;
  }
  let last: number;
  for (last = 0; last < 3; last = last + 1) {
    if (skip) {
      break;
    }
  }
  return total + last;
}

export function bareBlock(flag: boolean) {
  let direct: string | number = "start";
  {
    direct = 1;
  }
  const afterDirect: string = direct;

  let branched: string | number = "start";
  {
    if (flag) {
      branched = 1;
    }
  }
  const afterBranched: string = branched;

  let outer: string | number = "start";
  {
    let outer: string | number = "inner";
    outer = 1;
    const inside: string = outer;
  }
  const afterShadow: number = outer;
  return [afterDirect, afterBranched, afterShadow];
}
