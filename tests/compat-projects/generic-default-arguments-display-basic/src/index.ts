export interface Box<T = number, U = string> {
  value: T;
  label: U;
}

export class Response<R = Box<number, string>> {
  request!: R;
}

declare const box: Box;
declare const response: Response;

export const value: number = box.value;
export const label: string = box.label;
export const nested: number = response.request.value;

export const wrongBox: number = box;
export const wrongNested: string = response.request;
