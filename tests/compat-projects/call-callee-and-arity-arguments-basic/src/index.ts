declare const maybeFn: ((x: number) => void) | undefined;
declare const holder: { run?: () => void; count: number };
declare const either: ((a: string) => void) | ((a: string) => number) | undefined;

maybeFn(1);
maybeFn?.(1);
holder.run();
holder.run?.();
holder.count();
holder?.count();
maybeFn("x");
either("a");

declare function none(): void;
none(missingOne);
declare function pair(a: number, b: number): void;
pair(missingTwo);
pair(1, 2, missingThree, (x) => x);
