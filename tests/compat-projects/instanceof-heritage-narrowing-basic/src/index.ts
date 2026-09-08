declare class ClientError extends Error {
  readonly data: string;
}

interface Envelope {
  result: { error?: string };
}

interface Props {
  result: ClientError | Envelope;
}

export function levelFromPath(props: Props): 'error' | 'log' {
  return props.result instanceof Error ||
    ('error' in props.result.result && props.result.result.error)
    ? 'error'
    : 'log';
}

export function levelFromBinding(result: ClientError | Envelope): 'error' | 'log' {
  return result instanceof Error || ('error' in result.result && result.result.error)
    ? 'error'
    : 'log';
}

export function matchingBranchKeepsTheSubclass(result: ClientError | Envelope): string {
  return result instanceof Error ? result.data : (result.result.error ?? '');
}

interface ErrorShaped {
  name: string;
  message: string;
  stack?: string;
}

interface Tagged {
  tag: number;
}

export function aLookAlikeIsNotAnInstance(value: ErrorShaped | Tagged): string {
  if (value instanceof Error) {
    return value.message;
  }
  return 'tag' in value ? String(value.tag) : value.name;
}

export function theComplementIsExactlyTheOtherArm(
  result: ClientError | Envelope,
): string {
  return result instanceof Error ? '' : result.data;
}
