// @filename: a.ts
interface User { name: string; }
// @filename: b.ts
function f(user: User): string { return user.name; }
