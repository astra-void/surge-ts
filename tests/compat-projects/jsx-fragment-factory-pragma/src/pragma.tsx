/** @jsx dom */
import { dom } from "./renderer";

// An `@jsx` pragma without `@jsxFrag`: every fragment is TS17017.
export const fragment = <><span /></>;
export const element = <span />;
