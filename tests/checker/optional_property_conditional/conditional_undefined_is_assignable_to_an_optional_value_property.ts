interface Props { n?: number }
declare function take(props: Props): void;
declare const flag: boolean;
take({ n: flag ? 1 : undefined });
