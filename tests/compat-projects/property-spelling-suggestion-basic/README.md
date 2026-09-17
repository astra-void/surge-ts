# property-spelling-suggestion-basic

tsc's `reportNonexistentProperty` offers the closest member name
(`getSpellingSuggestion`: case-discounted Levenshtein, a length-difference
cap, names under three characters only when they differ by case) as TS2551,
and an element access with a literal key does the same on the key. surge
always reported TS2339.
