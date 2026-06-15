import { Child } from "pkg";

const c = new Child();

const okId: string = c.id;
const okLabel: string = c.label;
const okMethod: string = c.getId();

const badId: number = c.id;
const badMethod: number = c.getId();
