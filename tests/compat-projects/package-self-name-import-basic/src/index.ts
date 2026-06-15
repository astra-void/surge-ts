import { publicValue } from "app";
import { serverValue } from "app/server";

const ok1: string = publicValue;
const bad1: number = publicValue;
const ok2: number = serverValue;
const bad2: string = serverValue;
