declare global {
  namespace JSX {
    interface IntrinsicElements { div: any; span: any; p: any }
    interface Element {}
  }
}
const el = <div className="a"><span>hi</span></div>;
const frag = <><p /></>;
function App() { return <div />; }
export {};
