enum Flags {
  None = 0,
  Read = 1,
  Write = 2,
}

enum Names {
  Left = "left",
  Right = "right",
}

declare const anyNumber: number;
declare const anyString: string;

declare function takeFlags(flags: Flags): void;
declare function takeMember(flag: Flags.Read): void;
declare function takeNames(name: Names): void;

takeFlags(anyNumber);
takeFlags(Flags.Read | Flags.Write);
takeFlags(1);
takeMember(anyNumber);
takeNames(anyString);

export const combined: number = Flags.Read | Flags.Write;
