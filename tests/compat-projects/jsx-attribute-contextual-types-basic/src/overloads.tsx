interface Clickable { onClick: (which: "left" | "right") => void }
interface Linked { goTo: "home" | "contact" }
declare function Action(props: Clickable): JSX.Element;
declare function Action(props: Linked): JSX.Element;

const click = <Action onClick={which => which.length} />;
const link = <Action goTo="home" />;
const wrongClick = <Action onClick={which => which.toExponential()} />;
