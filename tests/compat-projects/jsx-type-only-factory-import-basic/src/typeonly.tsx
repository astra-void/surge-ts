import type React from 'react';

export function Box(props: { children: React.ReactNode }) {
    return <div className="box">{props.children}</div>;
}
export const wrong = <div className={1} />;
export const node: React.ReactNode = {};
