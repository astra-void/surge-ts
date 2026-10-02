const frozen = { x: 1 } as const;
frozen.x = 2;
export { frozen };
