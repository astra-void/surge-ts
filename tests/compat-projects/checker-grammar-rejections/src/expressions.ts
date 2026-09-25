const arrow = (a: number)
    => a;

enum Shifts {
    A = 1 << 32,
    B = 1 >> -33,
    C = (8 >>> 40),
    D = 1 << 31,
}
export { arrow, Shifts };
