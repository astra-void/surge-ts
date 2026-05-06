import { rootValue } from "root-types-pkg";
import { featureValue } from "exports-types-pkg/feature";
import { nextValue } from "exports-types-pkg/adapters/next";
import { stringDtsValue } from "exports-types-pkg/string-dts";
import { runtimeOnly } from "exports-types-pkg/runtime-only";
import { wildValue } from "exports-types-pkg/wild/feature";
import { missingNamed } from "exports-types-pkg/feature";
import { missingSubpath } from "exports-types-pkg/missing";
import { mtsValue } from "typed-mts-pkg";
import { ctsValue } from "typed-cts-pkg";

const a: string = rootValue;
const b: string = featureValue;
const c: string = nextValue;
const d: boolean = stringDtsValue;
const e: string = mtsValue;
const f: string = ctsValue;

void runtimeOnly;
void wildValue;
void missingNamed;
void missingSubpath;
