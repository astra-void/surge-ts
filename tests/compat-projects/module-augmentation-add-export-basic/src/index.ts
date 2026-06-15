import { makeClient } from "pkg";

const client = makeClient("c1");
const ok: string = client.id;
const bad: number = client.id;

makeClient(123);
