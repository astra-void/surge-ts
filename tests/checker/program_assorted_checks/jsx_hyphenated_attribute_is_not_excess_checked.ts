// @filename: example.tsx
// tsc never excess-checks hyphenated JSX attribute names (`data-slot`,
// `aria-*`), while a non-hyphenated unknown attribute still reports.
declare function Item(props: { label?: string }): null;
const ok = <Item data-slot="x" aria-bogus="y" />;
const bad = <Item dataslot="x" />;
