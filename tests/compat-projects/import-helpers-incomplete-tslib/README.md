# import-helpers-incomplete-tslib

`importHelpers` with a `tslib` that lacks helpers: each helper a file asks
for and `tslib` does not export is TS2343, once per file at the first
construct asking for it (`__rest`, `__exportStar`, `__importStar`), and a
pre-4.3 `__classPrivateFieldGet` taking too few parameters for the private
field transform is TS2807. The helpers `tslib` does declare
(`__awaiter`, a five-parameter `__classPrivateFieldSet`) report nothing.
