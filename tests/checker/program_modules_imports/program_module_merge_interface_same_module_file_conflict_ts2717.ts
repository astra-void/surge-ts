// Interfaces with the same name in one module file merge; a conflicting
// property type is reported once as TS2717 rather than a duplicate-identifier.
export {};
interface User { name: string; }
interface User { name: number; }
