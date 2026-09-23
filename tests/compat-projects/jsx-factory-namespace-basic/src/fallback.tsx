/** @jsx plain */
import { plain } from "./plain";

declare global {
    namespace JSX {
        interface IntrinsicElements {
            span: { title?: string };
        }
    }
}

export const fromGlobal = <span title="t"></span>;
export const notInGlobal = <div></div>;
