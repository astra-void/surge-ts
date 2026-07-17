type CardProps = {
  title: string;
  count?: number;
  children?: React.ReactNode;
  onSelect?: (event: React.MouseEvent) => void;
  ref?: (node: unknown) => void;
};

function Card({ title, count, children, onSelect }: CardProps) {
  return (
    <div id={title}>
      <button disabled={count === 0} onClick={onSelect}>
        {title}
      </button>
      {children}
    </div>
  );
}

const ok = (
  <Card
    title="inbox"
    count={2}
    ref={(node) => {
      void node;
    }}
    onSelect={(event) => {
      const x: number = event.clientX;
      void x;
    }}
  >
    <span>hello</span>
  </Card>
);

const badProp = <Card title={42} />;

export { ok, badProp };
