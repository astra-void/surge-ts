// @surge-compare: order
interface Opts { key: string; init: number; fn?: () => number }
declare function take(o: Opts): void;
take({ key: "k", fn: () => 1 });
