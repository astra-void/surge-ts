type MyAny = any;
type MyUnknown = unknown;
try {} catch (e: string) {}
try {} catch (e: string | number) {}
try {} catch (e: MyAny) {}
try {} catch (e: MyUnknown) {}
try {} catch (e: unknown) {}
try {} catch (e: any) {}
export {};
