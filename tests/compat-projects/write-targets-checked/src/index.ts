(missingCall()) = missingValue;
for (missingTarget() in missingSource) {}

declare const holder: { key: string; list: string[] };
for (holder.key in missingObject) { missingInBody; }
for (holder.missingMember of holder.list) {}
for ([missingFirst, missingSecond = missingDefault] of [["a"]]) {}

label: let labeled = 1;
if (labeled) let inIf = 2;
