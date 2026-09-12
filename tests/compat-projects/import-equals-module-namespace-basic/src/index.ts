import shapesModule = require("./shapes");

export const missingExport = shapesModule.nonExistentMember;

declare const valueMember: typeof shapesModule.thing;
export const missingValueMember = valueMember.nonExistentValueMember;

export const kind: number = shapesModule.thing.kind;
