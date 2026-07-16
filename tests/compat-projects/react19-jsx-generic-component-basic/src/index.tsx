import * as React from "react";

type ListProps<T> = {
  label: string;
  items: T[];
  renderItem: (item: T, index: number) => React.ReactNode;
  empty?: React.ReactNode;
};

function List<T>(props: ListProps<T>) {
  return <div>{props.empty}</div>;
}

const inferred = (
  <List
    label="numbers"
    items={[1, 2, 3]}
    renderItem={(item) => <span>{item.toFixed(2)}</span>}
  />
);

const explicit = (
  <List<string>
    label="letters"
    items={["a", "b"]}
    renderItem={(item, index) => item.toUpperCase() + String(index)}
    empty={<span>none</span>}
  />
);

const bad = (
  <List label={42} items={["a"]} renderItem={(item) => item} />
);

export { inferred, explicit, bad };
