const entityKind = Symbol.for('entityKind');

type EntityClass<T> =
  & ((abstract new (...args: any[]) => T) | (new (...args: any[]) => T))
  & { [entityKind]: string };

declare function is<T extends EntityClass<any>>(value: any, type: T): value is InstanceType<T>;

export class Sql<T = unknown> {
  static readonly [entityKind]: string = 'Sql';
  decoder = 1;
  declare _: { type: T };
}

export namespace Sql {
  export class Aliased<T = unknown> {
    static readonly [entityKind]: string = 'Sql.Aliased';
    fieldAlias = 'alias';
    declare _: { type: T };
  }
}

export class Column<T = unknown> {
  static readonly [entityKind]: string = 'Column';
  mapFromDriverValue(value: unknown): unknown {
    return value;
  }
  declare _: { type: T };
}

export function decoderOf(entry: Sql | Sql.Aliased | Column): unknown {
  if (is(entry, Sql)) {
    return entry.decoder;
  }
  if (is(entry, Sql.Aliased)) {
    return entry.fieldAlias;
  }
  return entry.mapFromDriverValue(null);
}

export class Dialect {
  static readonly [entityKind]: string = 'Dialect';
  readonly marker = 'dialect';
}

export interface DialectConfig {
  casing?: string;
}

export class QueryBuilder<T = unknown> {
  static readonly [entityKind]: string = 'QueryBuilder';
  dialect: Dialect | undefined;
  dialectConfig: DialectConfig | undefined;
  declare _: { type: T };

  constructor(dialect?: Dialect | DialectConfig) {
    this.dialect = is(dialect, Dialect) ? dialect : undefined;
    this.dialectConfig = is(dialect, Dialect) ? undefined : dialect;
  }
}

export function theGuardStillExcludesTheOtherMember(entry: Sql | Sql.Aliased): unknown {
  if (is(entry, Sql)) {
    return entry.fieldAlias;
  }
  return entry.fieldAlias;
}
