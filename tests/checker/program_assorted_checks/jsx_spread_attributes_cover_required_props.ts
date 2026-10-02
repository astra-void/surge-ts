// @filename: example.tsx
// A `{...spread}` whose type resolves to an object contributes its members, so
// a required prop carried by the spread is not reported missing (the shadcn
// wrapper idiom), while a spread without it still reports TS2741.
declare function Item(props: { label: string }): null;
declare const full: { label: string };
declare const partial: { id?: number };
const ok = <Item {...full} />;
const bad = <Item {...partial} />;
