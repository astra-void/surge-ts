declare function Picker<T>(props: { value: T; onChange: (value: T) => void }): JSX.Element;

const picked = <Picker value={1} onChange={value => value.toFixed()} />;
const wrongPick = <Picker value={1} onChange={value => value.toUpperCase()} />;
