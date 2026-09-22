declare function ps(): Promise<string>;
declare function pn(): Promise<number>;

export async function race() {
    const first = await Promise.race([ps(), pn()]);
    const asBoolean: boolean = first;
    const only = await Promise.race([ps()]);
    const asNumber: number = only;
    const settled = await Promise.race([ps(), pn()]);
    if (typeof settled === "string") {
        return settled.length;
    }
    return settled.toFixed();
}
