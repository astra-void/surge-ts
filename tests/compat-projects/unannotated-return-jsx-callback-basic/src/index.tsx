declare global {
  namespace JSX {
    interface IntrinsicElements {
      [name: string]: any;
    }
    interface Element {}
  }
}

import { Missing } from './not-there';

declare const AnyComponent: any;

export function ReturnsUnresolved() {
  return <Missing onPick={(value) => value} />;
}

export function ReturnsAny() {
  return <AnyComponent onPick={(value) => value} />;
}

function makeProvider<TShape>() {
  function Provider(props: { onEntries: (entries: TShape[]) => void }): JSX.Element {
    return props as unknown as JSX.Element;
  }
  return { Provider };
}

const stream = makeProvider<number>();

export function ReturnsTypedProvider() {
  return <stream.Provider onEntries={(entries) => entries} />;
}
