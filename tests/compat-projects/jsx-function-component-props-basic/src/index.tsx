type ButtonProps = {
  label: string;
  disabled?: boolean;
};

function Button(props: ButtonProps) {
  return <button disabled={props.disabled}>{props.label}</button>;
}

const ok = <Button label="Save" disabled />;
const badMissing = <Button />;
const badType = <Button label={123} />;
const badExtra = <Button label="Save" unknownProp="x" />;
