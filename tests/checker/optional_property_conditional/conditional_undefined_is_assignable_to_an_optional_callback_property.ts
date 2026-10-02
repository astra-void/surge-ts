// Without `exactOptionalPropertyTypes` an optional property's type includes
// `undefined`, so its contextual type must too: a conditional value is checked
// branch by branch, and the `undefined` branch has to be accepted.
interface Props { cb?: () => void }
declare function take(props: Props): void;
declare const flag: boolean;
take({ cb: flag ? () => {} : undefined });
