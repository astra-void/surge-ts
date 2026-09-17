import { hidden, helper, Shape } from "./withDefault";
import { onlyLocal, LocalType } from "./plain";
import fallback, { hidden as alias } from "./withDefault";
import { renamed, shown } from "./withDefault";
import { exported } from "./plain";

const value: number = renamed + shown + exported;
