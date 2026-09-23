declare function Button(props: { onPress: (x: "left" | "right") => void; label?: string }): JSX.Element;

const spread = <Button {...{ onPress: side => side.length }} />;
const mixed = <Button label="l" {...{ onPress: (side) => side }} />;
const wrongSpread = <Button {...{ onPress: side => side.toExponential() }} />;
