//! Which diagnostics tsc reports from its parser rather than its checker.
//!
//! tsc reports a program's syntactic diagnostics alone when there are any
//! (`GetDiagnosticsOfAnyProgram`): program and semantic diagnostics are never
//! computed for it.

/// Every code tsgo's parser and scanner report (`internal/parser`,
/// `internal/scanner`), sorted.
const TSC_PARSER_CODES: &[u32] = &[
    1002, 1003, 1005, 1007, 1010, 1011, 1012, 1034, 1068, 1069, 1084, 1109,
    1110, 1121, 1124, 1125, 1126, 1127, 1128, 1129, 1130, 1131, 1132, 1134,
    1135, 1136, 1137, 1138, 1139, 1140, 1142, 1144, 1145, 1146, 1160, 1161,
    1177, 1178, 1179, 1180, 1181, 1185, 1198, 1199, 1206, 1209, 1223, 1228,
    1260, 1327, 1328, 1351, 1352, 1353, 1357, 1359, 1369, 1381, 1382, 1385,
    1386, 1387, 1388, 1389, 1390, 1433, 1434, 1435, 1436, 1437, 1438, 1439,
    1440, 1441, 1442, 1443, 1453, 1472, 1477, 1478, 1486, 1487, 1488, 1489,
    1490, 1499, 1500, 1501, 1502, 1503, 1504, 1505, 1506, 1507, 1508, 1509,
    1510, 1511, 1512, 1513, 1514, 1515, 1516, 1517, 1518, 1519, 1520, 1521,
    1522, 1523, 1524, 1525, 1526, 1527, 1528, 1529, 1530, 1531, 1532, 1533,
    1534, 1535, 1536, 1537, 1538, 2427, 2457, 2657, 2754, 2809, 2819, 2880,
    6188, 6189, 8002, 8003, 8004, 8005, 8006, 8008, 8009, 8010, 8011, 8012,
    8013, 8016, 8017, 8033, 8034, 8037, 8038, 8039, 17002, 17006, 17007, 17008,
    17014, 17015, 17021, 18009, 18016, 18026, 18029, 18030, 18062, 18063,
];

pub fn is_tsc_parser_code(code: u32) -> bool {
    TSC_PARSER_CODES.binary_search(&code).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_codes_are_sorted_for_binary_search() {
        assert!(TSC_PARSER_CODES.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(is_tsc_parser_code(1005));
        assert!(!is_tsc_parser_code(2322));
    }
}
