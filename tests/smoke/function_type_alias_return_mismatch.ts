type Mapper = (value: string) => number;

function measure(value: string): string {
  return "1";
}

let mapper: Mapper = measure;
