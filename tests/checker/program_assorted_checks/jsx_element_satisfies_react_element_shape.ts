// @filename: example.tsx
// A JSX expression types as ReactElement<any, any> in tsc, so it satisfies a
// structurally-declared element shape (the react-hook-form `render` callback
// return); an empty opaque stub would miss the required members.
declare function take(render: () => { type: string; props: unknown; key: string | null }): void;
take(() => <div />);
