import * as React from "react";

const Ctx = React.createContext(0);

type DivProps = React.ComponentProps<"div">;
type Attrs = React.HTMLAttributes<HTMLDivElement>;
type BtnAttrs = React.ButtonHTMLAttributes<HTMLButtonElement>;

const node: React.ReactNode = "hello";

const a: DivProps = { className: "x" };
const b: Attrs = { id: "y", className: "z" };
const c: BtnAttrs = { disabled: true, id: "q" };

export { Ctx, node, a, b, c };
export type { DivProps, Attrs, BtnAttrs };
