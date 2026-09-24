if (Math.random()) { var hoisted = 1; }
for (var looped = 0; looped < 1; looped++) { }
for (var key in {}) { }
try { var tried = 1; } catch { var caught = 2; }
namespace TypesOnly { export interface Shape { s: string } }
namespace Values { export const v = 1; }
export { hoisted, looped, key, tried, caught, TypesOnly, Values };
export { Promise, PropertyKey };
export { missingName };
