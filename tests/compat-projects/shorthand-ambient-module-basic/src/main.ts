import $, { ajax } from "jquery";
import * as all from "jquery";
import legacy = require("jquery");
import styles from "./site.css";
import merged, { bar, other } from "merged";
import { known, missingMember } from "typed";
import missing from "not-declared";

$(ajax, all.anything, legacy.z, styles.root, merged, bar, other, known, missingMember, missing);
