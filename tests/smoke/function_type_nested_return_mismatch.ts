function getCount(): number {
  return 1;
}

function make(): () => string {
  return getCount;
}
