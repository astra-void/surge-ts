declare var scriptGlobal: number;
interface ScriptAndAugmented {
    s: string;
}
declare module "shared" {
    global {
        type SharedAlias = string;
        interface ScriptAndAugmented {
            t: string;
        }
    }
}
declare module "shared" {
    export { type SharedAlias };
}
