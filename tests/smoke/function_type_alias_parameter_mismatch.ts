type Mapper = (value: string) => number;

function length(value: number): number {
  return 1;
}

let mapper: Mapper = length;
