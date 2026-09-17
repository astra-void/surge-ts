const stringArrow = (x: number) => "w";
const widenedString: ReturnType<typeof stringArrow> = "other";

const numberArrow = () => 1;
const widenedNumber: ReturnType<typeof numberArrow> = 2;

const booleanArrow = () => true;
const widenedBoolean: ReturnType<typeof booleanArrow> = false;

const unionArrow = (c: boolean) => (c ? "a" : "b");
const keptUnion: ReturnType<typeof unionArrow> = "c";

const annotatedArrow = (): "a" => "a";
const keptAnnotated: ReturnType<typeof annotatedArrow> = "b";

const contextualLiteral: () => "a" = () => "a";
const keptContextual: ReturnType<typeof contextualLiteral> = "b";
