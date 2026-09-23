declare function Title(props: { title: string; level?: number }): JSX.Element;
declare function Plain(): JSX.Element;

// An excess attribute is the only report, not the missing prop as well.
const excess = <Title extra />;
// A mismatched value is reported alone: no excess, no missing prop.
const mismatch = <Title level="one" extra />;
// Nothing written: the missing prop against `IntrinsicAttributes & Props`.
const missing = <Title />;
const keyed = <Title key="k" title="t" />;
const ok = <Title title="t" level={1} />;
// A component with no parameter takes `IntrinsicAttributes` alone.
const plainExcess = <Plain value={1} />;
const plainOk = <Plain key="k" />;

// Intrinsic props are not joined with `IntrinsicAttributes`.
const boxMissing = <box />;
const boxExcess = <box width={1} depth={2} />;
const pairMissing = <pair />;
const hyphenated = <data data-count={1} label="x" />;
const hyphenatedExcess = <box width={1} aria-label="x" />;
