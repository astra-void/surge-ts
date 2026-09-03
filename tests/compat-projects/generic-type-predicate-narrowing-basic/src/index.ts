type INVALID = { status: "aborted" };
type DIRTY<T> = { status: "dirty"; value: T };
type OK<T> = { status: "valid"; value: T };
type SyncResult<T> = OK<T> | DIRTY<T> | INVALID;
type AnyResult<T> = SyncResult<T> | Promise<SyncResult<T>>;

const isValid = <T>(x: AnyResult<T>): x is OK<T> => (x as any).status === "valid";

export const handleResult = <Output>(result: SyncResult<Output>): Output | null => {
  if (isValid(result)) {
    return result.value;
  }
  return null;
};

type Sync = OK<string> | INVALID;
declare const isValidConst: (x: Sync) => x is OK<string>;

export function viaConstAnnotation(r: Sync): string | null {
  if (isValidConst(r)) {
    return r.value;
  }
  return null;
}

export function elseBranchKeepsTheOtherMember(r: Sync): string {
  if (isValidConst(r)) {
    return r.value;
  }
  return r.status;
}
