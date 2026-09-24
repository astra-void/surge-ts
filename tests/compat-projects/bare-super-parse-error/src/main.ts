class Base {
    method() {
        return 1;
    }
}
class Derived extends Base {
    method() {
        const bare = super;
        return super.method() + super["method"]();
    }
}
function outside() {
    return () => super;
}
export const gated: number = "not reported once the program has a parse error";
