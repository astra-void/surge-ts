export function early() {
    console.log(Regular.A);
    console.log(Inlined.B);
    deferred();
    return;
    function deferred() {
        console.log(Regular.A);
    }
    enum Regular { A }
    const enum Inlined { B }
}

export function late() {
    enum After { C }
    return After.C;
}
