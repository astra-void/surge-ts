import { current } from "versioned/current";
import { ancient } from "versioned/ancient";
import { ranged } from "versioned/ranged";
import { fromTs7 } from "legacy";
import { fromOld } from "legacy";

export const values = [current, ancient, ranged, fromTs7, fromOld];
