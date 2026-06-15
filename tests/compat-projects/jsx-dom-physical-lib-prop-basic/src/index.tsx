type LinkProps = {
  url: URL;
};

function Link(props: LinkProps) {
  return <div>{props.url.pathname}</div>;
}

const ok = <Link url={new URL("https://example.com")} />;
const bad = <Link url="https://example.com" />;
