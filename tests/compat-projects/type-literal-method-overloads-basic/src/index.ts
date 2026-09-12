type Chain<i> = {
  with<const p, c>(pattern: p, handler: (value: i) => c): Chain<i>;
  with<const p1, const p2, c>(
    p1: p1,
    p2: p2,
    handler: (value: i) => c,
  ): Chain<i>;
  with<const p, pred extends (value: i) => unknown, c>(
    pattern: p,
    predicate: pred,
    handler: (value: i) => c,
  ): Chain<i>;
  run(): i;
};

declare const chain: Chain<string>;

export const twoArguments = chain.with('a', (value) => value.length).run();

export const threeArguments = chain.with('a', 'b', (value) => value.length).run();

interface Iface {
  read(path: string): string;
  read(path: string, encoding: string): string;
}

declare const iface: Iface;

export const interfaceStillGroups = iface.read('a');

declare const inline: {
  read(path: string): string;
  read(path: string, encoding: string): string;
};

export const shorterArity = inline.read('a');

export const longerArity = inline.read('a', 'utf8');

export const assigningTheMergedReturnToNumberReports: number = inline.read('a');
