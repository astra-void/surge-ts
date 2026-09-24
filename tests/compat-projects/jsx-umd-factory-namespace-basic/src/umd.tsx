declare function Item(props: { task: string }): React.JSX.Element;

export const keyed = <Item key="k" task="t" />;
export const extra = <Item task="t" extra />;
export const intrinsic = <div className={1} />;
