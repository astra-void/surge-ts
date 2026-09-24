//! npm semver versions and ranges, ported from tsgo's `internal/semver`.
//!
//! Package resolution tests `typesVersions` keys and `types@<range>` export
//! conditions against the TypeScript version, so the range grammar has to
//! accept and reject exactly what tsc does: an unparsable range makes tsc skip
//! the entry rather than guess.

use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Version {
    major: u32,
    minor: u32,
    patch: u32,
    prerelease: Vec<String>,
}

impl Version {
    /// tsgo's `TryParseVersion`: `X`, `X.Y` or `X.Y.Z[-pre][+build]`, no
    /// wildcards and no leading zeroes.
    pub(crate) fn parse(text: &str) -> Option<Version> {
        let (core, prerelease, build) = split_qualifiers(text);
        let mut components = core.split('.');
        let major = numeric_component(components.next()?)?;
        let minor = components.next().map(numeric_component).unwrap_or(Some(0))?;
        let patch = components.next().map(numeric_component).unwrap_or(Some(0))?;
        if components.next().is_some() {
            return None;
        }
        let dots = core.matches('.').count();
        if dots < 2 && (prerelease.is_some() || build.is_some()) {
            return None;
        }
        let prerelease = match prerelease {
            Some(text) => {
                let parts: Vec<String> = text.split('.').map(str::to_string).collect();
                if !parts.iter().all(|part| is_prerelease_identifier(part)) {
                    return None;
                }
                parts
            }
            None => Vec::new(),
        };
        if let Some(build) = build
            && !build.split('.').all(|part| !part.is_empty() && part.bytes().all(is_identifier_byte))
        {
            return None;
        }
        Some(Version {
            major,
            minor,
            patch,
            prerelease,
        })
    }

    fn release(major: u32, minor: u32, patch: u32) -> Version {
        Version {
            major,
            minor,
            patch,
            prerelease: Vec::new(),
        }
    }

    fn increment_major(&self) -> Version {
        Version::release(self.major + 1, 0, 0)
    }

    fn increment_minor(&self) -> Version {
        Version::release(self.major, self.minor + 1, 0)
    }

    fn increment_patch(&self) -> Version {
        Version::release(self.major, self.minor, self.patch + 1)
    }

    fn with_prerelease_zero(mut self) -> Version {
        self.prerelease = vec!["0".to_string()];
        self
    }

    fn compare(&self, other: &Version) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then_with(|| compare_prerelease(&self.prerelease, &other.prerelease))
    }
}

fn compare_prerelease(left: &[String], right: &[String]) -> Ordering {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => {
            for (left, right) in left.iter().zip(right) {
                let ordering = compare_prerelease_identifier(left, right);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            left.len().cmp(&right.len())
        }
    }
}

fn compare_prerelease_identifier(left: &str, right: &str) -> Ordering {
    if left == right {
        return Ordering::Equal;
    }
    match (is_numeric_identifier(left), is_numeric_identifier(right)) {
        (true, true) => match (left.parse::<u32>(), right.parse::<u32>()) {
            (Ok(left_number), Ok(right_number)) => left_number.cmp(&right_number),
            _ => left.len().cmp(&right.len()).then_with(|| left.cmp(right)),
        },
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => left.cmp(right),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operator {
    Less,
    LessEqual,
    Equal,
    GreaterEqual,
    Greater,
}

#[derive(Debug, Clone)]
struct Comparator {
    operator: Operator,
    operand: Version,
}

impl Comparator {
    fn test(&self, version: &Version) -> bool {
        let ordering = version.compare(&self.operand);
        match self.operator {
            Operator::Less => ordering == Ordering::Less,
            Operator::LessEqual => ordering != Ordering::Greater,
            Operator::Equal => ordering == Ordering::Equal,
            Operator::GreaterEqual => ordering != Ordering::Less,
            Operator::Greater => ordering == Ordering::Greater,
        }
    }
}

/// A disjunction of comparator sets; an empty disjunction or an empty set
/// matches every version, as `*` does.
#[derive(Debug, Clone)]
pub(crate) struct VersionRange {
    alternatives: Vec<Vec<Comparator>>,
}

impl VersionRange {
    /// tsgo's `TryParseVersionRange`.
    pub(crate) fn parse(text: &str) -> Option<VersionRange> {
        let mut alternatives = Vec::new();
        for range in text.trim().split("||") {
            let range = range.trim();
            if range.is_empty() {
                continue;
            }
            let comparators = match hyphen_bounds(range) {
                Some((left, right)) => parse_hyphen(left, right)?,
                None => {
                    let mut comparators = Vec::new();
                    for simple in range.split_ascii_whitespace() {
                        let (operator, operand) = split_simple(simple)?;
                        comparators.extend(parse_comparator(operator, operand)?);
                    }
                    comparators
                }
            };
            alternatives.push(comparators);
        }
        Some(VersionRange { alternatives })
    }

    pub(crate) fn test(&self, version: &Version) -> bool {
        self.alternatives.is_empty()
            || self
                .alternatives
                .iter()
                .any(|comparators| comparators.iter().all(|comparator| comparator.test(version)))
    }
}

/// `^\s*([a-z0-9-+.*]+)\s+-\s+([a-z0-9-+.*]+)\s*$`, case-insensitively.
fn hyphen_bounds(range: &str) -> Option<(&str, &str)> {
    let mut words = range.split_ascii_whitespace();
    let left = words.next()?;
    let dash = words.next()?;
    let right = words.next()?;
    if words.next().is_some() || dash != "-" {
        return None;
    }
    (is_range_operand(left) && is_range_operand(right)).then_some((left, right))
}

/// `^([~^<>=]|<=|>=)?\s*([a-z0-9-+.*]+)$` on one whitespace-free simple range.
fn split_simple(simple: &str) -> Option<(&str, &str)> {
    let operator_len = if simple.starts_with("<=") || simple.starts_with(">=") {
        2
    } else if simple.starts_with(['~', '^', '<', '>', '=']) {
        1
    } else {
        0
    };
    let (operator, operand) = simple.split_at(operator_len);
    is_range_operand(operand).then_some((operator, operand))
}

fn is_range_operand(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'+' | b'.' | b'*'))
}

struct PartialVersion {
    version: Version,
    major_is_wildcard: bool,
    minor_is_wildcard: bool,
    patch_is_wildcard: bool,
}

/// tsgo's `parsePartial`: `xr ( '.' xr ( '.' xr qualifier? )? )?` where a
/// missing component is a wildcard.
fn parse_partial(text: &str) -> Option<PartialVersion> {
    let (core, prerelease, build) = split_qualifiers(text);
    let mut components = core.split('.');
    let major_text = components.next()?;
    let minor_text = components.next();
    let patch_text = components.next();
    if components.next().is_some() {
        return None;
    }
    if patch_text.is_none() && (prerelease.is_some() || build.is_some()) {
        return None;
    }
    for component in [Some(major_text), minor_text, patch_text].into_iter().flatten() {
        if !is_wildcard(component) && numeric_component(component).is_none() {
            return None;
        }
    }
    if prerelease
        .into_iter()
        .chain(build)
        .any(|qualifier| {
            qualifier.is_empty()
                || !qualifier.bytes().all(|byte| is_identifier_byte(byte) || byte == b'.')
        })
    {
        return None;
    }
    let major_is_wildcard = is_wildcard(major_text);
    let minor_is_wildcard = minor_text.is_none_or(is_wildcard);
    let patch_is_wildcard = patch_text.is_none_or(is_wildcard);
    let component = |text: Option<&str>| text.and_then(numeric_component).unwrap_or(0);
    let mut version = if major_is_wildcard {
        Version::release(0, 0, 0)
    } else if minor_is_wildcard {
        Version::release(component(Some(major_text)), 0, 0)
    } else if patch_is_wildcard {
        Version::release(component(Some(major_text)), component(minor_text), 0)
    } else {
        Version::release(
            component(Some(major_text)),
            component(minor_text),
            component(patch_text),
        )
    };
    if let Some(prerelease) = prerelease {
        version.prerelease = prerelease.split('.').map(str::to_string).collect();
    }
    Some(PartialVersion {
        version,
        major_is_wildcard,
        minor_is_wildcard,
        patch_is_wildcard,
    })
}

fn parse_hyphen(left: &str, right: &str) -> Option<Vec<Comparator>> {
    let left = parse_partial(left)?;
    let right = parse_partial(right)?;
    let mut comparators = Vec::new();
    if !left.major_is_wildcard {
        comparators.push(Comparator {
            operator: Operator::GreaterEqual,
            operand: left.version,
        });
    }
    if !right.major_is_wildcard {
        let (operator, operand) = if right.minor_is_wildcard {
            (Operator::Less, right.version.increment_major())
        } else if right.patch_is_wildcard {
            (Operator::Less, right.version.increment_minor())
        } else {
            (Operator::LessEqual, right.version)
        };
        comparators.push(Comparator { operator, operand });
    }
    Some(comparators)
}

fn parse_comparator(operator: &str, operand: &str) -> Option<Vec<Comparator>> {
    let partial = parse_partial(operand)?;
    let any_wildcard = partial.minor_is_wildcard || partial.patch_is_wildcard;
    if partial.major_is_wildcard {
        return Some(if operator == "<" || operator == ">" {
            vec![Comparator {
                operator: Operator::Less,
                operand: Version::release(0, 0, 0).with_prerelease_zero(),
            }]
        } else {
            Vec::new()
        });
    }
    let version = partial.version;
    let comparators = match operator {
        "~" => {
            let upper = if partial.minor_is_wildcard {
                version.increment_major()
            } else {
                version.increment_minor()
            };
            vec![
                Comparator {
                    operator: Operator::GreaterEqual,
                    operand: version,
                },
                Comparator {
                    operator: Operator::Less,
                    operand: upper,
                },
            ]
        }
        "^" => {
            let upper = if version.major > 0 || partial.minor_is_wildcard {
                version.increment_major()
            } else if version.minor > 0 || partial.patch_is_wildcard {
                version.increment_minor()
            } else {
                version.increment_patch()
            };
            vec![
                Comparator {
                    operator: Operator::GreaterEqual,
                    operand: version,
                },
                Comparator {
                    operator: Operator::Less,
                    operand: upper,
                },
            ]
        }
        "<" | ">=" => {
            let operand = if any_wildcard {
                version.with_prerelease_zero()
            } else {
                version
            };
            let operator = if operator == "<" {
                Operator::Less
            } else {
                Operator::GreaterEqual
            };
            vec![Comparator { operator, operand }]
        }
        "<=" | ">" => {
            let inclusive_upper = operator == "<=";
            let (operator, operand) = if partial.minor_is_wildcard {
                (
                    if inclusive_upper {
                        Operator::Less
                    } else {
                        Operator::GreaterEqual
                    },
                    version.increment_major().with_prerelease_zero(),
                )
            } else if partial.patch_is_wildcard {
                (
                    if inclusive_upper {
                        Operator::Less
                    } else {
                        Operator::GreaterEqual
                    },
                    version.increment_minor().with_prerelease_zero(),
                )
            } else {
                (
                    if inclusive_upper {
                        Operator::LessEqual
                    } else {
                        Operator::Greater
                    },
                    version,
                )
            };
            vec![Comparator { operator, operand }]
        }
        _ => {
            if any_wildcard {
                let upper = if partial.minor_is_wildcard {
                    version.increment_major()
                } else {
                    version.increment_minor()
                };
                vec![
                    Comparator {
                        operator: Operator::GreaterEqual,
                        operand: version.with_prerelease_zero(),
                    },
                    Comparator {
                        operator: Operator::Less,
                        operand: upper.with_prerelease_zero(),
                    },
                ]
            } else {
                vec![Comparator {
                    operator: Operator::Equal,
                    operand: version,
                }]
            }
        }
    };
    Some(comparators)
}

/// Splits `core[-pre][+build]`; the prerelease starts at the first `-` of
/// the text before any `+`.
fn split_qualifiers(text: &str) -> (&str, Option<&str>, Option<&str>) {
    let (head, build) = match text.split_once('+') {
        Some((head, build)) => (head, Some(build)),
        None => (text, None),
    };
    match head.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease), build),
        None => (head, None, build),
    }
}

/// `0|[1-9]\d*`, within `u32`.
fn numeric_component(text: &str) -> Option<u32> {
    if !is_numeric_identifier(text) {
        return None;
    }
    text.parse().ok()
}

fn is_numeric_identifier(text: &str) -> bool {
    !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'))
}

fn is_prerelease_identifier(text: &str) -> bool {
    is_numeric_identifier(text)
        || (!text.is_empty()
            && !text.as_bytes()[0].is_ascii_digit()
            && text.bytes().all(is_identifier_byte))
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-'
}

fn is_wildcard(text: &str) -> bool {
    matches!(text, "*" | "x" | "X")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(range: &str, version: &str) -> bool {
        VersionRange::parse(range)
            .expect("range parses")
            .test(&Version::parse(version).expect("version parses"))
    }

    #[test]
    fn comparators_follow_npm_semantics() {
        assert!(matches(">=4", "7.0.2"));
        assert!(!matches("<4", "7.0.2"));
        assert!(matches("*", "7.0.2"));
        assert!(matches("7", "7.0.2"));
        assert!(!matches("7.1", "7.0.2"));
        assert!(matches("^7.0.0", "7.0.2"));
        assert!(matches("~7.0", "7.0.2"));
        assert!(!matches(">7.0", "7.0.2"));
        assert!(matches("<=7.0", "7.0.2"));
        assert!(matches("<5.0 || >=7.0", "7.0.2"));
        assert!(matches("6.0 - 7.0", "7.0.2"));
        assert!(!matches("6.0 - 6.9", "7.0.2"));
    }

    #[test]
    fn prereleases_sort_below_their_release() {
        assert!(!matches(">=7.0.2", "7.0.2-dev"));
        assert!(matches(">=7.0", "7.0.2-dev"));
    }

    #[test]
    fn malformed_ranges_do_not_parse() {
        assert!(VersionRange::parse(">= 4").is_none());
        assert!(VersionRange::parse("latest").is_none());
        assert!(VersionRange::parse("01.2").is_none());
    }
}
