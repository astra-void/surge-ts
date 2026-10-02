// Without `noImplicitAny` the literal is plainly `never[]`.
export function f() { const out = []; out.push(1); }
