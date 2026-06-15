import { User } from "./user";

const ok = new User("a");
const bad = new User(123);

const idOk: string = ok.id;
const idBad: number = ok.id;
