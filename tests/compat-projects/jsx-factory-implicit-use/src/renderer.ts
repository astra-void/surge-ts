export function h(..._args: unknown[]): unknown { return null; }
export function Fragment(..._args: unknown[]): unknown { return null; }
export const React = { createElement: h, Fragment };
declare global {
    namespace JSX {
        interface IntrinsicElements { [name: string]: unknown }
        interface Element {}
    }
}
