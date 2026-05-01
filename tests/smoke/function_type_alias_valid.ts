type Mapper = (value: string) => number;

function length(value: string): number {
  return 1;
}

let mapper: Mapper = length;
