// A spread of a nominally-typed source (`{ ...defaults, ...props }` where both
// are `Props`) contributes the reference's members instead of being skipped, so
// destructured names resolve rather than reporting TS2339 on `{}`.
interface Props { url: string; email: string }
const defaults: Props = { url: "u", email: "e" };
function render(props: Props = defaults): string {
const { url, email } = { ...defaults, ...props };
return url + email;
}
