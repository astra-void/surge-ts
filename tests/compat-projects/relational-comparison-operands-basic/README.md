# relational-comparison-operands-basic

tsc's `checkBinaryLikeExpression` for `<`, `>`, `<=` and `>=` accepts two
numeric operands (`number | bigint`, in any mix) or two non-numeric operands
comparable to each other, literals compared by their base type. surge
accepted only number/number and string/string, so `bigint > bigint`,
`Date < Date` and `boolean < boolean` were false TS2365s.
