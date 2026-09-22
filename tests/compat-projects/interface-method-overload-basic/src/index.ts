interface Runner {
    run(a: string, b: number): void;
    run(a: number): void;
}
interface Pair {
    set(key: string, value: string): void;
    set(key: number, value: boolean): void;
}
declare const runner: Runner;
declare const pair: Pair;

runner.run(true);
runner.run("a", "b");
runner.run(1);
runner.run("a", 1);
pair.set(true, "x");
pair.set("k", true);
pair.set(1, true);
export {};
