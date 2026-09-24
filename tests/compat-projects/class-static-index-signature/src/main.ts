class Registry {
    static readonly [key: string]: number;
    static readonly [key: number]: 42 | 233;
}
export const byName: number = Registry["anything"];
export const byDot: number = Registry.other;
export const byIndex: 42 | 233 = Registry[2];

class Plain {
    static known = 1;
}
Plain.known;
Plain.missing;
