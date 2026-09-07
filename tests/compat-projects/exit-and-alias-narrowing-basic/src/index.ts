declare const proc: { exit(code?: number): never };
declare function fail(message: string): never;
declare function use(value: string): void;

export function afterBareNeverCall(file: string | undefined): void {
  if (!file) {
    fail('missing');
  }
  use(file);
}

export function afterMemberNeverCall(file: string | undefined): void {
  if (!file) {
    use('reporting');
    proc.exit(1);
  }
  use(file);
}

export function aliasedGuard(name: string | undefined): void {
  const named = name !== undefined && name.length > 0;
  if (named) {
    use(name);
  }
}

type Message = { id: number } & (
  | { direction: 'down'; result: string }
  | { direction: 'up' }
);

export function destructuredDiscriminant(message: Message): string {
  const { direction } = message;
  if (direction === 'up') {
    return 'up';
  }
  return message.result;
}

export function aliasedDiscriminant(message: Message): string {
  const direction = message.direction;
  if (direction !== 'up') {
    return message.result;
  }
  return 'up';
}

export function aliasKeepsItsOwnNarrowing(options: { value?: string }): void {
  const { value } = options;
  if (value) {
    use(value);
  }
}

export function reassignedAliasIsNotAGuard(name: string | undefined): void {
  let named = name !== undefined;
  named = true;
  if (named) {
    use(name);
  }
}
