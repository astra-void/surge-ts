export const matcher = Symbol.for('@fixture/matcher');
export type matcher = typeof matcher;

interface Matchable {
  [matcher](): { matched: boolean };
  label: string;
}

export type MatcherResult = Matchable[matcher];

declare const value: Matchable;

export const readThroughValue = value[matcher]();

export const readDeclaredMember = value.label;

export const absentMemberStillReports = value.missing;
