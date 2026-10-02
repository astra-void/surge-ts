interface Id { name: string } interface Spec { imported: Id } export function f(spec: Spec) { if (spec.imported.name === 'a') { spec.imported.name = 'b'; } }
