import * as React from "react";

type ButtonProps = React.ButtonAttributes;

function Button(props: ButtonProps) {
    return <button disabled={props.disabled} />;
}

const el = <Button onClick={(event) => { const flag: boolean = event.clientX; }} />;

export { el };
