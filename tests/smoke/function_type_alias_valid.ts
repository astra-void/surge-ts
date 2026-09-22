type Mapper = (value: string) => number;

function measure(value: string): number {
  return 1;
}

let mapper: Mapper = measure;
