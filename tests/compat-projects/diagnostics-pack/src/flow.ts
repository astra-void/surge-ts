export {};
// TS2448, TS2454
function test_flow() {
    let a = block_val; // TS2448
    let block_val = 1;

    let unassigned: number;
    let b = unassigned; // TS2454
}
