import { server } from "pkg/server";
import { feature } from "pkg/features/auth";

const ok1: boolean = server.ready;
const bad1: string = server.ready;
const ok2: string = feature.name;
const bad2: number = feature.name;
