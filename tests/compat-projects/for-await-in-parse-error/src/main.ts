declare const source: Record<string, number>;
export async function walk() {
    for await (const key in source) {
        console.log(key);
    }
}
export const gated: number = "not reported once the program has a parse error";
