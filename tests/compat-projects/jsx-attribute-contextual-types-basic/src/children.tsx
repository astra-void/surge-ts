declare function List(props: { items: string[]; children: (item: string, index: number) => JSX.Element }): JSX.Element;

const list = <List items={["a"]}>{(item, index) => <li key={index}>{item}</li>}</List>;
const wrongChild = <List items={["a"]}>{(item) => <li>{item.toExponential()}</li>}</List>;

declare function Render(props: { children: (index: number) => string }): JSX.Element;

const render = <Render>{(index) => index}</Render>;
const renderOk = <Render>{(index) => index.toFixed()}</Render>;
