export const direct: number = () => {
  return 1;
};

const count = () => {
  return 1;
};
export const counted: 1 = count();
export const counter: string = count;

const describe = (flag: boolean) => {
  if (flag) {
    return { label: 'on' };
  }
  return { label: 'off' };
};
export const label: number = describe(true).label;

const makeContext = ({ user }: { user?: string }) => {
  const resolveUser = () => {
    if (!user) {
      return null;
    }
    return { name: user };
  };
  return { user: resolveUser() };
};
export const context: string = makeContext({});

type Envelope =
  | { type: 'started' }
  | { type: 'state'; state: 'idle' }
  | { type: 'state'; state: 'connecting'; error: Error };

declare const envelopes: { result: Envelope }[];
const connecting = envelopes
  .map((envelope) => envelope.result)
  .filter((result) => result.type === 'state' && result.state === 'connecting');
export const lastError: string = connecting[0]!.error;
