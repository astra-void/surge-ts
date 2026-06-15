type BaseProps = {
  id: string;
};

type ChildrenProps = {
  children?: string;
};

type Props = BaseProps & ChildrenProps;

function Component(props: Props) {
  const id: string = props.id;
  const child: string | undefined = props.children;
}

Component({ id: "x" });
Component({ id: "x", children: "ok" });
Component({ children: "missing id" });
