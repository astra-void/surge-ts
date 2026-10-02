// @surge-compare: spans
interface User { id: number; name: string; active: boolean; } type InvalidPick = Pick<User, "id" | "missing">; let invalidPickUsage: InvalidPick;
