type Action = { kind: "A"; payload: number } | { kind: "B"; payload: string };
declare function load(): Action;
export function viaParameter({ kind, payload }: Action) {
  if (kind === "A") { const n: string = payload; return n; }
  const s: number = payload;
  return s;
}
export function viaConst(action: Action) {
  const { kind, payload } = action;
  if (kind === "A") { const n: string = payload; return n; }
  const viaTernary: boolean = kind === "B" ? payload : payload;
  return viaTernary;
}
export function viaCall() {
  const { kind, payload } = load();
  if (kind !== "A") { const n: number = payload; return n; }
  const s: string = payload;
  return s;
}
export function viaSwitch({ kind, payload }: Action) {
  switch (kind) {
    case "A": { const n: string = payload; return n; }
    case "B": { const n: number = payload; return n; }
  }
}
export function notDependent(action: Action) {
  const kind = action.kind;
  const payload = action.payload;
  if (kind === "A") { const n: boolean = payload; return n; }
  return 0;
}
export function withDefault({ kind, payload = 1 }: { kind: "A"; payload?: number } | { kind: "B"; payload?: string }) {
  if (kind === "A") { const n: boolean = payload; return n; }
  return 0;
}
