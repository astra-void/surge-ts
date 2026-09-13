/// <reference types="@vendor/dual-flavor-pkg" />

export function describe(handle: VendorHandle, options: VendorOptions): string {
  return `${handle.id}:${options.retries}`;
}

export function theGlobalIsTheDeclarationFlavor(handle: VendorHandle): number {
  return handle.retries;
}
