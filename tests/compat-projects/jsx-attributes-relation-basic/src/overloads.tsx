declare function Choice(props: { a: string }): JSX.Element;
declare function Choice(props: { b: number; c?: boolean }): JSX.Element;

const first = <Choice a="x" />;
const second = <Choice b={1} />;
const neitherMissing = <Choice />;
const neitherWrong = <Choice b="x" />;
const neitherExcess = <Choice b={1} d />;
