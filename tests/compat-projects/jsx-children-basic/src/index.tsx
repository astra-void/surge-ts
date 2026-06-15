type TextProps = {
  children: string;
};

function Text(props: TextProps) {
  return <div>{props.children}</div>;
}

const ok = <Text>hello</Text>;
const bad = <Text>{123}</Text>;
const missing = <Text />;
