function C(x: number) {
    return x;
}
namespace C {
    export var count = 1;
}
namespace C {
    export function label() {
        return "";
    }
}
export const called: number = C(2);
export const text: string = C.label();
export const total: number = C.count;
export const missing = C.absent;
export const constructed = new C(2);
