type ButtonProps = {
  label: string;
};

const UI = {
  Button(props: ButtonProps) {
    return <button>{props.label}</button>;
  },
};

const ok = <UI.Button label="Save" />;
const bad = <UI.Button label={123} />;
