// @filename: example.tsx
// A callable object used as a JSX component (a `forwardRef`/`memo`-style exotic
// component, which is callable rather than a bare function) has its props checked
// through its call signature.
interface Btn { (props: { label: string }): null; }
declare const Button: Btn;
const bad = <Button label={123} />;
