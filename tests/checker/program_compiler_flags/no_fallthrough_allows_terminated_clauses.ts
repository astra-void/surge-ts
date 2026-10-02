// @noFallthroughCasesInSwitch: true
export function a(x: number) { switch (x) { case 1: return; case 2: throw x; default: break; } }
