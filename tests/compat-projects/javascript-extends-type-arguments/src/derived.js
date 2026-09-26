import { Box, Pair } from "./bases";

export class Bare extends Box {}

/** @extends {Box} */
export class TagWithoutArguments extends Box {}

/** @extends {Box<number>} */
export class Typed extends Box {}

/** @extends {Box<number, string>} */
export class TooMany extends Box {}

export class Defaulted extends Pair {}

/** @extends {Pair<number>} */
export class PairTyped extends Pair {}
