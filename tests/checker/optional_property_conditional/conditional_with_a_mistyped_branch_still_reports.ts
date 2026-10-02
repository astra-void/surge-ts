// The widening must not swallow a genuinely wrong branch.
interface Props { n?: number }
declare function take(props: Props): void;
declare const flag: boolean;
take({ n: flag ? "x" : undefined });
