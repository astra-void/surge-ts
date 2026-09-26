declare const h: any;
declare global {
    namespace JSX {
        interface Element {}
        interface IntrinsicElements { [name: string]: any }
    }
}

declare const items: string[];
declare const frozen: readonly string[];
declare const pair: [string, string];
declare const maybe: string[] | undefined;
declare const loose: any;

// A spread child must be an array: TS2609 otherwise, at the container.
export const fromArray = <div>{...items}</div>;
export const fromReadonly = <div>{...frozen}</div>;
export const fromAny = <div>{...loose}</div>;
export const fromTuple = <div>{...pair}</div>;
export const fromOptional = <div>{...maybe}</div>;

// `jsxFactory` without `jsxFragmentFactory`: every fragment is TS17016.
export const fragment = <>{...items}</>;
export const nested = <div><><span /></></div>;
