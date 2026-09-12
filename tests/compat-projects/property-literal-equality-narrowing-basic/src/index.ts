type QueryTypeFilter = 'all' | 'active' | 'inactive';

interface Filters {
  type?: QueryTypeFilter;
  refetchType?: QueryTypeFilter | 'none';
}

declare function refetch(filters: { type: QueryTypeFilter }): Promise<void>;

// The early return removes `'none'` from `filters?.refetchType` for the rest
// of the function, so the `??` chain settles on `QueryTypeFilter`.
export function invalidate(filters?: Filters): Promise<void> {
  if (filters?.refetchType === 'none') {
    return Promise.resolve();
  }
  return refetch({ type: filters?.refetchType ?? filters?.type ?? 'active' });
}

// The matching branch narrows the property to the literal itself.
export function onlyNone(filters: Filters): 'none' | undefined {
  if (filters.refetchType === 'none') {
    return filters.refetchType;
  }
  return undefined;
}

// A write after a narrowing read still targets the declared type.
export class ParseStatus {
  value: 'aborted' | 'dirty' | 'valid' = 'valid';
  dirty(): void {
    if (this.value === 'valid') this.value = 'dirty';
  }
  abort(): void {
    if (this.value !== 'aborted') this.value = 'aborted';
  }
}

// The narrowing is real, not a suppression: `'none'` is gone, the rest stays.
interface Written {
  mode?: 'all' | 'none';
}
export function stillReports(written: Written): void {
  if (written.mode !== 'none') {
    const bad: 'none' = written.mode;
    void bad;
  }
}
