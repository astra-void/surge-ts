import { x } from "pkg/subpath";
const y: number = x;

// subpath-pkg
import { featureValue } from "subpath-pkg/feature";
import { nestedValue as subpathNestedValue } from "subpath-pkg/nested/path";

let a: string = featureValue;
let b: number = subpathNestedValue;
let mismatch: number = featureValue; // should be TS2322, not TS2307

// exports-types-pkg
import { rootValue } from "exports-types-pkg";
import { featureValue as exportsFeatureValue } from "exports-types-pkg/feature";
import { nestedValue } from "exports-types-pkg/nested/path";
import { stringDtsValue } from "exports-types-pkg/string-dts";
import { runtimeOnly } from "exports-types-pkg/runtime-only";

let okRoot: string = rootValue;
let okFeature: string = exportsFeatureValue;
let okNested: number = nestedValue;
let okStringDts: boolean = stringDtsValue;

// runtime-only should remain unresolved
let unresolvedUse = runtimeOnly;

// wildcard export should remain unsupported/unresolved
import { wildValue } from "exports-types-pkg/wild/wild";
let unresolvedWild = wildValue;

// missing exports should emit TS2305
import { missingNamed } from "exports-types-pkg/feature";
import missingDefault from "exports-types-pkg/feature";

// scoped pkg subpath
import { helper } from "@scope/subtool/helpers";
let okScope: string = helper();