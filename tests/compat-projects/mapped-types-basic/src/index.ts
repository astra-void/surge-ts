const config = {
  mode: "dev",
  retries: 3,
  enabled: true,
} as const;

type Config = typeof config;
type ConfigClone = { [K in keyof Config]: Config[K] };
type ConfigOptional = { [K in keyof Config]?: Config[K] };

const okConfig: ConfigClone = {
  mode: "dev",
  retries: 3,
  enabled: true,
};

const wrongMode: ConfigClone = {
  mode: "prod",
  retries: 3,
  enabled: true,
};

const wrongRetries: ConfigClone = {
  mode: "dev",
  retries: 4,
  enabled: true,
};

const missingEnabled: ConfigClone = {
  mode: "dev",
  retries: 3,
};

const extraConfig: ConfigClone = {
  mode: "dev",
  retries: 3,
  enabled: true,
  extra: true,
};

const optionalOk: ConfigOptional = {
  mode: "dev",
};

const optionalWrong: ConfigOptional = {
  mode: "prod",
};

type User = {
  name: string;
  age: number;
};

type Clone<T> = { [K in keyof T]: T[K] };
type OptionalClone<T> = { [K in keyof T]?: T[K] };

type UserClone = Clone<User>;
type OptionalUserClone = OptionalClone<User>;

const okUser: UserClone = {
  name: "Ada",
  age: 37,
};

const wrongUserName: UserClone = {
  name: 123,
  age: 37,
};

const missingUserAge: UserClone = {
  name: "Ada",
};

const optionalUserOk: OptionalUserClone = {
  name: "Ada",
};

const optionalUserWrong: OptionalUserClone = {
  age: "old",
};

