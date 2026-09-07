type ProcedureType = 'query' | 'mutation' | 'subscription';

export function readProcedureType(raw: string): ProcedureType | null {
  if (raw === 'query' || raw === 'mutation' || raw === 'subscription') {
    return raw;
  }
  return null;
}

export function pickBranch(x: 'a' | 'b'): 'a' {
  if (x === 'a') {
    return x;
  }
  return 'a';
}

export function pickComplement(x: 'a' | 'b'): 'b' {
  if (x === 'a') {
    return 'b';
  }
  return x;
}

export function counts(n: number): 1 | 2 | null {
  return n === 1 || n === 2 ? n : null;
}

type Target = 'draft-04' | 'draft-07';

export function retarget(input: Target): Target {
  let target: Target = input;
  if (target === 'draft-04') {
    target = 'draft-07';
  }
  if (target === 'draft-04') {
    return target;
  }
  return target;
}

export function stillWide(raw: string): string {
  if (raw === 'q') {
    return raw;
  }
  return raw;
}
