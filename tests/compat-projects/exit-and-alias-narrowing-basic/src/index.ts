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

export function reassignedAliasIsNotAGuard(name: string | undefined): void {
  let named = name !== undefined;
  named = true;
  if (named) {
    use(name);
  }
}
