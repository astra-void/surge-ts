type Known = { id: string };
type Mixed = Known & MissingType;

const value: Mixed = { id: "x" } as any;
const id: string = value.id;
const bad: number = value.id;
