/// <reference types="plug" />
import type { Req, Local, Other } from 'fx';

declare const req: Req;
declare const local: Local;
declare const other: Other;

export const declaredMember: string = req.id;
export const augmentedMember: boolean = req.ws;
export const indexMember = req.headers.host;
export const localDeclared: number = local.y;
export const localAugmented: number = local.z;
export const untouched: number = other.x;

export const wrongDeclared: number = req.id;
export const wrongAugmented: number = req.ws;
