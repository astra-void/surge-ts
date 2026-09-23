type Texts = { kind: "text"; onChange: (value: string) => void };
type Counts = { kind?: "count"; onChange: (value: number) => void };
declare function Field(props: Texts | Counts): JSX.Element;

const text = <Field kind="text" onChange={value => value.toUpperCase()} />;
const count = <Field kind="count" onChange={value => value.toFixed()} />;
const implied = <Field onChange={value => value.toFixed()} />;
const undetermined = <Field kind={undefined} onChange={value => value.toFixed()} />;
const wrongText = <Field kind="text" onChange={value => value.toExponential()} />;
