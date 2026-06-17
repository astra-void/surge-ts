import { ns, renamedAlpha, sourceDefault, namespaceAlias } from "./barrel";

export const fromNamespace: number = ns.alpha;
export const fromNamespaceFn: number = ns.make();
export const fromRenamed: number = renamedAlpha;
export const fromDefault: number = sourceDefault;
export const fromAlias: string = namespaceAlias.beta;
