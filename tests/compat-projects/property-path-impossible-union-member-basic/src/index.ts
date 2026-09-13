export interface ArrayMember {
  longValue?: never;
  arrayValue: { stringValues?: string[]; longValues?: number[] };
}

export interface LongMember {
  longValue: number;
  arrayValue?: never;
}

export type Field = ArrayMember | LongMember;

export function readField(field: Field): unknown {
  if (field.longValue !== undefined) {
    return field.longValue;
  } else if (field.arrayValue !== undefined) {
    if (field.arrayValue.stringValues !== undefined) {
      return field.arrayValue.stringValues;
    }
    if (field.arrayValue.longValues !== undefined) {
      return field.arrayValue.longValues;
    }
    throw new Error('unknown array type');
  }
  return undefined;
}

export function aMemberThatSurvivesIsStillKept(field: Field): unknown {
  if (field.arrayValue === undefined) {
    return field.longValue;
  }
  return field.arrayValue;
}

export function theOtherMemberIsGoneInTheNarrowedBranch(field: Field): unknown {
  if (field.arrayValue !== undefined) {
    const stillANumber: number = field.longValue;
    return stillANumber;
  }
  return 0;
}
