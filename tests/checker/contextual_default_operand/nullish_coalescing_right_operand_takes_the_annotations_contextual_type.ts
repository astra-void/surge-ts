// `override ?? ((options) => …)` — the tRPC/react-query option-default shape.
// The annotation's contextual type reaches the right operand, so the fallback
// arrow's parameters are typed instead of implicit any.
// @noImplicitAny: true
type OnSuccess = (options: { originalFn: () => void }) => void;
declare const override: OnSuccess | undefined;
const onSuccess: OnSuccess = override ?? ((options) => options.originalFn());
void onSuccess;
