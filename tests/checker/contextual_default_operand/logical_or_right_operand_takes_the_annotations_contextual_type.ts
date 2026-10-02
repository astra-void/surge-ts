// @noImplicitAny: true
type OnSuccess = (options: { originalFn: () => void }) => void;
declare const override: OnSuccess | undefined;
const onSuccess: OnSuccess = override || ((options) => options.originalFn());
void onSuccess;
