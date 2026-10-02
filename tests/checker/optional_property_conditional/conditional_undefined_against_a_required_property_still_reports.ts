// A required property keeps rejecting `undefined`; optionality is what admits it.
interface Props { n: number }
declare function take(props: Props): void;
declare const flag: boolean;
take({ n: flag ? 1 : undefined });
