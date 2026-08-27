import { make } from "libmod";

const outer = make();
const viaSiblings: string = outer.inner().deep().value;

declare const carrier: FixtureNS.Carrier;
const picked = carrier.pick("a");
const viaGlobal: string = picked.value;
const badLevel = carrier.pick("c");

export { viaSiblings, viaGlobal, badLevel };
