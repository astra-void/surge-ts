type Mapper = (value: string) => number;

function measure(value: number): number {
  return 1;
}

let mapper: Mapper = measure;
