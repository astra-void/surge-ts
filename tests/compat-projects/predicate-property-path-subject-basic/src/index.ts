const entityKind: unique symbol = Symbol.for('entityKind');

type EntityClass<T> =
  & ((abstract new (...args: any[]) => T) | (new (...args: any[]) => T))
  & { [entityKind]: string };

declare function is<T extends EntityClass<any>>(value: any, type: T): value is InstanceType<T>;

export class Role {
  static readonly [entityKind]: string = 'Role';
  constructor(readonly name: string) {}
}

export function fromAPropertyPath(options: { role: Role | string }): string {
  return is(options.role, Role) ? options.role.name : options.role;
}

export function fromAPropertyPathInAnIf(options: { role: Role | string }): string {
  if (is(options.role, Role)) {
    return options.role.name;
  }
  return options.role;
}

export function fromABindingStillWorks(role: Role | string): string {
  return is(role, Role) ? role.name : role;
}

export function theOtherBranchIsTheOtherMember(options: { role: Role | string }): string {
  return is(options.role, Role) ? options.role.name : options.role.name;
}
