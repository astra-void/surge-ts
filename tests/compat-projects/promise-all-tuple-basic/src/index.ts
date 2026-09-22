declare function ps(): Promise<string>;
declare function pn(): Promise<number>;

export async function all() {
    const [a, b] = await Promise.all([ps(), pn()]);
    const x: number = a;
    const y: string = b;
    const mixed = await Promise.all([ps(), 1]);
    const z: boolean = mixed[1];
    const list = await Promise.all(["a", "b"].map(() => pn()));
    const w: string = list[0];
    return a.length + b.toFixed() + list.length;
}
