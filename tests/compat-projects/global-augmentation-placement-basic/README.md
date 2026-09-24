# global-augmentation-placement-basic

`checkModuleDeclaration` for `global { … }`: without `declare` outside an
ambient context it is TS2670; anywhere it is not an augmentation of an
external module — the top level of a script, a namespace body, or an ambient
module inside a module file — it is TS2669. Inside an applied augmentation
`checkModuleAugmentationElement` rejects export lists, `export *`, `export
default` (TS2666) and imports of modules (TS2667); `import x = N.y` and
exported declarations are fine.
