type ButtonProps = {
  label: string;
};

function Button(props: ButtonProps) {
  return <button>{props.label}</button>;
}

const ok = <Button label="Save" />;
const bad = <Button label={123} />;

export { ok, bad };
