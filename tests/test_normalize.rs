#![allow(clippy::inconsistent_digit_grouping)]
#![allow(clippy::needless_collect)]
#![allow(clippy::too_many_arguments)]

//! 规范化边界回归语料。
//!
//! 本文件把三件经常被混为一谈的事情拆成独立断言：
//!
//! 1. 字符串往返：`parse(text)` 得到的对象其 `Display` 是否等于原文本。
//! 2. 结构相等：`parse(a) == parse(b)` / `Hash` 是否一致，字段级身份是什么。
//! 3. matches 等价：两个 `VersionReq` 在一组固定 probe version 上的匹配向量。
//!
//! 第 3 点只是"有限见证"：向量相同只说明在该 probe 集合上不可区分，不说明两个
//! 要求全域等价。`finite_witness_same_vector_but_not_globally_equivalent` 专门给出向量相同但全域不等价的反例。
//!
//! 每个样例分别断言：parse 是否成功（失败时附错误文本）、Display 文本、把
//! Display 文本再次 parse 后的结构、（版本样例的）排序关系，以及对统一 probe
//! 版本库的匹配向量。任何一条断言失败都会带上输入字符串与全部实际值。

use semver::{BuildMetadata, Comparator, Op, Prerelease, Version, VersionReq};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[track_caller]
fn version(input: &str) -> Version {
    Version::parse(input).unwrap_or_else(|err| {
        panic!("expected Version::parse({input:?}) to succeed, got error: {err}")
    })
}

#[track_caller]
fn req(input: &str) -> VersionReq {
    VersionReq::parse(input).unwrap_or_else(|err| {
        panic!("expected VersionReq::parse({input:?}) to succeed, got error: {err}")
    })
}

#[track_caller]
fn comparator(input: &str) -> Comparator {
    Comparator::parse(input).unwrap_or_else(|err| {
        panic!("expected Comparator::parse({input:?}) to succeed, got error: {err}")
    })
}

#[track_caller]
fn hash64(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// 把匹配向量渲染成 0/1 字符串，例如 "10110"，方便失败时一眼对照。
#[track_caller]
fn match_vector(req: &VersionReq, probes: &[Version]) -> String {
    probes
        .iter()
        .map(|probe| if req.matches(probe) { '1' } else { '0' })
        .collect()
}

#[track_caller]
fn comparator_vector(cmp: &Comparator, probes: &[Version]) -> String {
    probes
        .iter()
        .map(|probe| if cmp.matches(probe) { '1' } else { '0' })
        .collect()
}

// --------------------------------------------------------------------------
// 统一 probe version 库。
//
// 索引 0..=23 是稳定版；24..=31 是 prerelease；31 号额外携带 build metadata，
// 用来锁定 build 对 matches 不可见的契约。新增样例时只能追加，不能插在中间，
// 因为语料里的 expected 向量按下标书写。
// --------------------------------------------------------------------------

fn probes() -> Vec<Version> {
    let stable = [
        "0.0.0",
        "0.0.1",
        "0.0.2",
        "0.0.3", // 0..=3
        "0.1.0",
        "0.1.9", // 4..=5
        "0.2.0",
        "0.2.5", // 6..=7
        "0.3.0", // 8
        "1.0.0",
        "1.0.1",
        "1.0.9", // 9..=11
        "1.2.0",
        "1.2.3",
        "1.2.4",
        "1.2.5",                    // 12..=15
        "1.3.0",                    // 16
        "2.0.0",                    // 17
        "18446744073709551615.0.0", // 18 (u64::MAX 主版本, 超大整数边界)
        "0.9.0",
        "0.10.0", // 19..=20 (1.19 vs 1.5 的小版本数值序)
        "1.0.10",
        "1.10.0", // 21..=22 (1.0.10 vs 1.10.0)
        "2.0.1",  // 23
    ];
    let prerelease = [
        "0.0.0-alpha",          // 24
        "0.0.3-alpha",          // 25
        "0.2.0-alpha",          // 26
        "1.0.0-alpha",          // 27
        "1.2.3-alpha",          // 28
        "1.2.3-alpha.1",        // 29
        "1.2.4-alpha",          // 30
        "1.2.3-alpha+build.42", // 31 (带 build: 匹配行为必须与 28 相同)
    ];
    stable
        .into_iter()
        .chain(prerelease)
        .map(Version::parse)
        .map(Result::unwrap)
        .collect()
}

// ==========================================================================
// Version: parse / Display / 再解析
// ==========================================================================

struct VersionCase {
    /// 原始输入。
    input: &'static str,
    /// Display 后的规范化文本；None 表示期望 parse 失败。
    display: Option<&'static str>,
    /// 当输入本身就不规范（被 Display 改写）时，记录原始文本；否则留空。
    original_spelling: bool,
}

const VERSION_CASES: &[VersionCase] = &[
    // --- 基本往返：Display 后文本与输入一致 ---
    VersionCase {
        input: "0.0.0",
        display: Some("0.0.0"),
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3",
        display: Some("1.2.3"),
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3-alpha1",
        display: Some("1.2.3-alpha1"),
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3+build5",
        display: Some("1.2.3+build5"),
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3-1+1",
        display: Some("1.2.3-1+1"),
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3-1-1+1-1-1",
        display: Some("1.2.3-1-1+1-1-1"),
        original_spelling: false,
    },
    VersionCase {
        input: "1.1.0-beta-10",
        display: Some("1.1.0-beta-10"),
        original_spelling: false,
    },
    // 单独连字符是合法 identifier。
    VersionCase {
        input: "1.0.0--",
        display: Some("1.0.0--"),
        original_spelling: false,
    },
    VersionCase {
        input: "1.0.0+-",
        display: Some("1.0.0+-"),
        original_spelling: false,
    },
    // build 允许前导零，且 Display 原样保留；prerelease 的 01 带字母时也允许。
    VersionCase {
        input: "1.2.3+001",
        display: Some("1.2.3+001"),
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3+00",
        display: Some("1.2.3+00"),
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3-0a+05build",
        display: Some("1.2.3-0a+05build"),
        original_spelling: false,
    },
    VersionCase {
        input: "0.4.0-beta.1+0851523",
        display: Some("0.4.0-beta.1+0851523"),
        original_spelling: false,
    },
    // 超大整数：u64::MAX 合法；identifier 中的长数字串不按 u64 解析也合法。
    VersionCase {
        input: "18446744073709551615.18446744073709551615.18446744073709551615",
        display: Some("18446744073709551615.18446744073709551615.18446744073709551615"),
        original_spelling: false,
    },
    VersionCase {
        input: "1.0.0-99999999999999999999999",
        display: Some("1.0.0-99999999999999999999999"),
        original_spelling: false,
    },
    // --- parse 必须失败：错误文本一并锁定 ---
    VersionCase {
        input: "",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1.2",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "07.0.0",
        display: None,
        original_spelling: false,
    }, // 主版本前导零
    VersionCase {
        input: "1.01.0",
        display: None,
        original_spelling: false,
    }, // 次版本前导零
    VersionCase {
        input: "1.0.00",
        display: None,
        original_spelling: false,
    }, // 修订号前导零
    VersionCase {
        input: "1.2.3-01",
        display: None,
        original_spelling: false,
    }, // 纯数字 prerelease 前导零
    VersionCase {
        input: "1.2.3-1.01",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3-",
        display: None,
        original_spelling: false,
    }, // 空 prerelease
    VersionCase {
        input: "1.2.3+",
        display: None,
        original_spelling: false,
    }, // 空 build
    VersionCase {
        input: "1.2.3-+x",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3-alpha+",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3++",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3-alpha..1",
        display: None,
        original_spelling: false,
    }, // 空段
    // ASCII 非法字符（下划线、非 ASCII、NUL、空白、多余点）。
    VersionCase {
        input: "1.2.3-alpha_1",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3+build_42",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3\u{e9}",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3-rc\u{0}",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: " 1.2.3",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1.2.3 ",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "1. 2.3",
        display: None,
        original_spelling: false,
    },
    // 超大整数溢出：三个数字位置各自检测。
    VersionCase {
        input: "18446744073709551616.0.0",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "0.18446744073709551616.0",
        display: None,
        original_spelling: false,
    },
    VersionCase {
        input: "0.0.18446744073709551616",
        display: None,
        original_spelling: false,
    },
];

struct VersionErrorCase {
    input: &'static str,
    error: &'static str,
}

const VERSION_ERRORS: &[VersionErrorCase] = &[
    VersionErrorCase {
        input: "",
        error: "empty string, expected a semver version",
    },
    VersionErrorCase {
        input: "1",
        error: "unexpected end of input while parsing major version number",
    },
    VersionErrorCase {
        input: "1.2",
        error: "unexpected end of input while parsing minor version number",
    },
    VersionErrorCase {
        input: "07.0.0",
        error: "invalid leading zero in major version number",
    },
    VersionErrorCase {
        input: "1.01.0",
        error: "invalid leading zero in minor version number",
    },
    VersionErrorCase {
        input: "1.0.00",
        error: "invalid leading zero in patch version number",
    },
    VersionErrorCase {
        input: "1.2.3-01",
        error: "invalid leading zero in pre-release identifier",
    },
    VersionErrorCase {
        input: "1.2.3-",
        error: "empty identifier segment in pre-release identifier",
    },
    VersionErrorCase {
        input: "1.2.3+",
        error: "empty identifier segment in build metadata",
    },
    VersionErrorCase {
        input: "1.2.3-+x",
        error: "empty identifier segment in pre-release identifier",
    },
    VersionErrorCase {
        input: "1.2.3-alpha+",
        error: "empty identifier segment in build metadata",
    },
    VersionErrorCase {
        input: "1.2.3++",
        error: "empty identifier segment in build metadata",
    },
    VersionErrorCase {
        input: "1.2.3-alpha..1",
        error: "empty identifier segment in pre-release identifier",
    },
    VersionErrorCase {
        input: "1.2.3-alpha_1",
        error: "unexpected character '_' after pre-release identifier",
    },
    VersionErrorCase {
        input: "1.2.3+build_42",
        error: "unexpected character '_' after build metadata",
    },
    VersionErrorCase {
        input: "1.2.3\u{e9}",
        error: "unexpected character '\u{e9}' after patch version number",
    },
    VersionErrorCase {
        input: " 1.2.3",
        error: "unexpected character ' ' while parsing major version number",
    },
    VersionErrorCase {
        input: "1.2.3 ",
        error: "unexpected character ' ' after patch version number",
    },
    VersionErrorCase {
        input: "18446744073709551616.0.0",
        error: "value of major version number exceeds u64::MAX",
    },
    VersionErrorCase {
        input: "0.18446744073709551616.0",
        error: "value of minor version number exceeds u64::MAX",
    },
    VersionErrorCase {
        input: "0.0.18446744073709551616",
        error: "value of patch version number exceeds u64::MAX",
    },
];

#[test]
fn version_table_parse_display_reparse() {
    for case in VERSION_CASES {
        let parsed = match Version::parse(case.input) {
            Ok(parsed) => parsed,
            Err(err) => {
                assert!(
                    case.display.is_none(),
                    "input {:?} unexpectedly failed to parse: {err}",
                    case.input,
                );
                continue;
            }
        };

        let expected_display = case
            .display
            .unwrap_or_else(|| panic!("input {:?} parsed unexpectedly: {parsed}", case.input));

        // 断言 1: Display 规范化文本。
        let actual_display = parsed.to_string();
        assert_eq!(
            actual_display,
            expected_display,
            "Display mismatch for input {:?}: expected {expected_display:?}, got {actual_display:?}",
            case.input,
        );

        // 断言 2: 再次 parse 得到结构相等的对象（规范化输出必须仍可解析且幂等）。
        let reparsed = Version::parse(&actual_display).unwrap_or_else(|err| {
            panic!(
                "normalized form {actual_display:?} of input {:?} failed to reparse: {err}",
                case.input,
            )
        });
        assert_eq!(
            reparsed, parsed,
            "reparse of normalized form {actual_display:?} (input {:?}) is not structurally equal",
            case.input,
        );

        // 断言 3: 字符串往返是独立概念。只有原样拼写才应与输入逐字符相同；
        // 被规范化的输入（本语料里 Version 没有此类成功案例，VersionReq 有）
        // 会在这里被显式标记。
        assert_eq!(
            actual_display == case.input,
            !case.original_spelling,
            "round-trip expectation inconsistent for input {:?}",
            case.input,
        );

        // 断言 4: 规范化幂等——第二次 Display 必须完全相同。
        assert_eq!(
            reparsed.to_string(),
            actual_display,
            "Display is not idempotent for input {:?}",
            case.input,
        );
    }
}

#[test]
fn version_table_error_messages() {
    for case in VERSION_ERRORS {
        let err = Version::parse(case.input)
            .unwrap_err_or_else(|| panic!("input {:?} unexpectedly parsed", case.input));
        assert_eq!(
            err.to_string(),
            case.error,
            "error text mismatch for input {:?}",
            case.input,
        );
    }
}

/// 局部辅助，避免给全文件引入 trait 扩展方法。
trait UnwrapErrOrElse<T> {
    fn unwrap_err_or_else(self, f: impl FnOnce() -> T) -> semver::Error;
}

impl<T> UnwrapErrOrElse<T> for Result<T, semver::Error> {
    #[track_caller]
    fn unwrap_err_or_else(self, f: impl FnOnce() -> T) -> semver::Error {
        match self {
            Ok(_) => {
                f();
                unreachable!("closure above is expected to panic")
            }
            Err(err) => err,
        }
    }
}

// ==========================================================================
// Version 的 Ord 与 cmp_precedence：严格递增链 + 相邻关系
// ==========================================================================

/// 按 `Ord`（含 build metadata）严格递增排列的链。
///
/// 覆盖 prerelease 数值/字母差异（pre.2 < pre.11 但 pre2 > pre11）、
/// 空 prerelease 最大、数字 identifier 小于字母 identifier，以及 build 的
/// 独立次序（包括允许的前导零）。
const TOTAL_ORDER_CHAIN: &[&str] = &[
    "0.9.0",
    "0.10.0", // 数值序而非 ASCII 序
    "1.0.0-alpha",
    "1.0.0-alpha.2",
    "1.0.0-alpha.11", // 点分隔时 2 < 11（数值 identifier）
    "1.0.0-alpha11",  // 无点字母段按 ASCII：alpha11 < alpha2
    "1.0.0-alpha2",
    "1.0.0-beta",
    "1.0.0-rc.1",
    "1.0.0-x.7",
    "1.0.0-x.23",
    "1.0.0-x.1a0",
    "1.0.0-x.a", // 数字 identifier 恒小于非数字 identifier
    "1.0.0",
    "1.0.0+0",
    "1.0.0+00", // build 允许前导零：0 < 00 < 1
    "1.0.0+1",
    "1.0.0+0a", // 纯数字 identifier < 含字母 identifier
    "1.0.0+build.1",
    "1.0.0+build.2",
    "1.0.0+build.10", // build 数字段也按数值
];

#[test]
fn version_total_order_chain() {
    let parsed: Vec<Version> = TOTAL_ORDER_CHAIN.iter().map(|s| version(s)).collect();
    for window in parsed.windows(2) {
        let (left, right) = (&window[0], &window[1]);
        assert!(
            left < right,
            "total order violated: {left} (from {:?}) is not less than {right} (from {:?})",
            TOTAL_ORDER_CHAIN[parsed.iter().position(|v| v == left).unwrap_or(0)],
            TOTAL_ORDER_CHAIN[parsed.iter().position(|v| v == right).unwrap_or(0)],
        );
    }
}

/// 同一 major.minor.patch 与 prerelease 下，仅 build 不同的版本：
/// `cmp_precedence` 必须相等，但 `Ord` 必须有区别。
const PRECEDENCE_EQUAL_BUT_TOTAL_ORDER_DIFF: &[&str] =
    &["1.0.0", "1.0.0+00", "1.0.0+build.42", "1.0.0+zzz"];

#[test]
fn precedence_ignores_build_but_total_order_does_not() {
    let parsed: Vec<Version> = PRECEDENCE_EQUAL_BUT_TOTAL_ORDER_DIFF
        .iter()
        .map(|s| version(s))
        .collect();
    for left in &parsed {
        for right in &parsed {
            assert_eq!(
                left.cmp_precedence(right),
                std::cmp::Ordering::Equal,
                "cmp_precedence must ignore build metadata: {left} vs {right}",
            );
        }
    }
    // 但 Ord 把 build 纳入比较：链内 build 文本不同的两两必须不全相等。
    for (i, left) in parsed.iter().enumerate() {
        for (j, right) in parsed.iter().enumerate() {
            if i != j {
                assert_ne!(
                    left, right,
                    "structural Eq must distinguish differing build: {left} vs {right}",
                );
            }
        }
    }
    // build 的独立次序：00 < build.42 < zzz（ASCII）。
    assert!(parsed[1] < parsed[2] && parsed[2] < parsed[3]);
}

// ==========================================================================
// Comparator: parse / Display / 字段身份 / 匹配向量
// ==========================================================================

/// Comparator 的结构身份由 (op, major, minor, patch, pre) 五个字段决定；
/// build metadata 根本不进入 Comparator 结构体（parse 时丢弃）。
struct ComparatorCase {
    input: &'static str,
    /// Display 规范化文本；None 表示期望 parse 失败。
    display: Option<&'static str>,
    op: Op,
    major: u64,
    minor: Option<u64>,
    patch: Option<u64>,
    pre: &'static str,
    /// 对 probes() 的匹配向量。
    vector: &'static str,
}

impl ComparatorCase {
    const fn new(
        input: &'static str,
        display: Option<&'static str>,
        op: Op,
        major: u64,
        minor: Option<u64>,
        patch: Option<u64>,
        pre: &'static str,
        vector: &'static str,
    ) -> Self {
        ComparatorCase {
            input,
            display,
            op,
            major,
            minor,
            patch,
            pre,
            vector,
        }
    }
}

const COMPARATOR_CASES: &[ComparatorCase] = &[
    // 缺省 op 规范化为 caret；缺省位保留 None。
    ComparatorCase::new(
        "1",
        Some("^1"),
        Op::Caret,
        1,
        None,
        None,
        "",
        "00000000011111111000011000000000",
    ),
    ComparatorCase::new(
        "1.2",
        Some("^1.2"),
        Op::Caret,
        1,
        Some(2),
        None,
        "",
        "00000000000011111000001000000000",
    ),
    // 显式 operator 保留；缺省 minor/patch 语义在 matches 中体现。
    ComparatorCase::new(
        "=1",
        Some("=1"),
        Op::Exact,
        1,
        None,
        None,
        "",
        "00000000011111111000011000000000",
    ),
    ComparatorCase::new(
        ">=1.2.3",
        Some(">=1.2.3"),
        Op::GreaterEq,
        1,
        Some(2),
        Some(3),
        "",
        "00000000000001111110001100000000",
    ),
    ComparatorCase::new(
        "<2.0.0",
        Some("<2.0.0"),
        Op::Less,
        2,
        Some(0),
        Some(0),
        "",
        "11111111111111111001111000000000",
    ),
    ComparatorCase::new(
        "~1.2.3",
        Some("~1.2.3"),
        Op::Tilde,
        1,
        Some(2),
        Some(3),
        "",
        "00000000000001110000000000000000",
    ),
    ComparatorCase::new(
        "^1.2.3",
        Some("^1.2.3"),
        Op::Caret,
        1,
        Some(2),
        Some(3),
        "",
        "00000000000001111000001000000000",
    ),
    // 零主版本 caret：^0 只覆盖 0.x；^0.2 只覆盖 0.2.x。
    ComparatorCase::new(
        "^0",
        Some("^0"),
        Op::Caret,
        0,
        None,
        None,
        "",
        "11111111100000000001100000000000",
    ),
    ComparatorCase::new(
        "^0.2",
        Some("^0.2"),
        Op::Caret,
        0,
        Some(2),
        None,
        "",
        "00000011000000000000000000000000",
    ),
    ComparatorCase::new(
        "^0.0",
        Some("^0.0"),
        Op::Caret,
        0,
        Some(0),
        None,
        "",
        "11110000000000000000000000000000",
    ),
    ComparatorCase::new(
        "^0.0.3",
        Some("^0.0.3"),
        Op::Caret,
        0,
        Some(0),
        Some(3),
        "",
        "00010000000000000000000000000000",
    ),
    // wildcard：default op 被替换为 Wildcard，Display 补回 ".*"。
    ComparatorCase::new(
        "1.*",
        Some("1.*"),
        Op::Wildcard,
        1,
        None,
        None,
        "",
        "00000000011111111000011000000000",
    ),
    ComparatorCase::new(
        "1.2.*",
        Some("1.2.*"),
        Op::Wildcard,
        1,
        Some(2),
        None,
        "",
        "00000000000011110000000000000000",
    ),
    ComparatorCase::new(
        "1.x.x",
        Some("1.*"),
        Op::Wildcard,
        1,
        None,
        None,
        "",
        "00000000011111111000011000000000",
    ),
    // 显式 operator + wildcard：op 保留，"*" 在 Display 中消失。
    ComparatorCase::new(
        "=1.*",
        Some("=1"),
        Op::Exact,
        1,
        None,
        None,
        "",
        "00000000011111111000011000000000",
    ),
    ComparatorCase::new(
        ">1.*",
        Some(">1"),
        Op::Greater,
        1,
        None,
        None,
        "",
        "00000000000000000110000100000000",
    ),
    // prerelease：字段保留，Display 不丢。
    ComparatorCase::new(
        ">=1.2.3-alpha",
        Some(">=1.2.3-alpha"),
        Op::GreaterEq,
        1,
        Some(2),
        Some(3),
        "alpha",
        "00000000000001111110001100001101",
    ),
    ComparatorCase::new(
        "1.2.3-2.3.4",
        Some("^1.2.3-2.3.4"),
        Op::Caret,
        1,
        Some(2),
        Some(3),
        "2.3.4",
        "00000000000001111000001000001101",
    ),
    // build 在 parse 时即被丢弃：结构与 Display 都没有它，匹配也不看它。
    ComparatorCase::new(
        "=1.2.3+build",
        Some("=1.2.3"),
        Op::Exact,
        1,
        Some(2),
        Some(3),
        "",
        "00000000000001000000000000000000",
    ),
    // 缺省位 + 显式 operator：~1.2 只锁 minor。
    ComparatorCase::new(
        "~1.2",
        Some("~1.2"),
        Op::Tilde,
        1,
        Some(2),
        None,
        "",
        "00000000000011110000000000000000",
    ),
];

const COMPARATOR_ERRORS: &[(&str, &str)] = &[
    ("1.0.0-01", "invalid leading zero in pre-release identifier"),
    ("1.0.0+4.", "empty identifier segment in build metadata"),
    (
        ">",
        "unexpected end of input while parsing major version number",
    ),
    (
        "1.",
        "unexpected end of input while parsing minor version number",
    ),
    ("1.*.", "unexpected character after wildcard in version req"),
    (
        "1.*.1",
        "unexpected character after wildcard in version req",
    ),
    (
        "1.2.3+4\u{ff}",
        "unexpected character '\u{ff}' after build metadata",
    ),
    (
        "@1.0.0",
        "unexpected character '@' while parsing major version number",
    ),
];

#[test]
fn comparator_table() {
    let probes = probes();
    for case in COMPARATOR_CASES {
        let parsed = comparator(case.input);

        let actual_display = parsed.to_string();
        assert_eq!(
            actual_display,
            case.display.unwrap(),
            "Display mismatch for comparator input {:?}",
            case.input,
        );

        // 字段级身份断言：五项分开比较，不用一个 assert_eq 糊过去。
        assert_eq!(parsed.op, case.op, "op field mismatch for {:?}", case.input);
        assert_eq!(
            parsed.major, case.major,
            "major field mismatch for {:?}",
            case.input
        );
        assert_eq!(
            parsed.minor, case.minor,
            "minor field mismatch for {:?}",
            case.input
        );
        assert_eq!(
            parsed.patch, case.patch,
            "patch field mismatch for {:?}",
            case.input
        );
        assert_eq!(
            parsed.pre.as_str(),
            case.pre,
            "pre field mismatch for {:?}",
            case.input
        );

        // Display 再 parse：结构必须相同，且 build 永远回不来。
        let reparsed = comparator(&actual_display);
        assert_eq!(
            reparsed, parsed,
            "reparsed comparator differs for {:?}",
            case.input
        );
        assert_eq!(
            reparsed.to_string(),
            actual_display,
            "Display not idempotent for comparator {:?}",
            case.input,
        );

        let actual_vector = comparator_vector(&parsed, &probes);
        assert_eq!(
            actual_vector,
            case.vector,
            "comparator match vector mismatch for {:?}\nprobes in order: {:?}",
            case.input,
            probes.iter().map(Version::to_string).collect::<Vec<_>>(),
        );
    }

    for (input, expected_error) in COMPARATOR_ERRORS {
        let err = Comparator::parse(input).unwrap_err();
        assert_eq!(
            err.to_string(),
            *expected_error,
            "error mismatch for {input:?}"
        );
    }
}

// ==========================================================================
// VersionReq: parse / Display / comparator 序列身份 / 匹配向量
// ==========================================================================

struct ReqCase {
    input: &'static str,
    /// Display 规范化文本；None 表示期望 parse 失败。
    display: Option<&'static str>,
    /// 逐个 comparator 的规范化 Display（同时锁定顺序这一身份维度）。
    comparators: &'static [&'static str],
    vector: &'static str,
}

macro_rules! req_case {
    ($input:literal, $display:literal, [$($cmp:literal),*], $vector:literal) => {
        ReqCase {
            input: $input,
            display: Some($display),
            comparators: &[$($cmp),*],
            vector: $vector,
        }
    };
}

const REQ_CASES: &[ReqCase] = &[
    // 裸通配符 -> 空 comparator 列表，Display 固定为 "*"。
    req_case!("*", "*", [], "11111111111111111111111100000000"),
    req_case!("x", "*", [], "11111111111111111111111100000000"),
    req_case!("X", "*", [], "11111111111111111111111100000000"),
    // 缺省 op -> caret，且 Display 会把 caret 显式写出来（规范化）。
    req_case!("1", "^1", ["^1"], "00000000011111111000011000000000"),
    req_case!("1.2", "^1.2", ["^1.2"], "00000000000011111000001000000000"),
    // 多 comparator：逗号统一为 ", "，operator 周围空白被吃掉。
    req_case!(
        ">= 1.0.0, <2.0.0",
        ">=1.0.0, <2.0.0",
        [">=1.0.0", "<2.0.0"],
        "00000000011111111000011000000000"
    ),
    req_case!(
        "  >=1.0.0 , <2.0.0  ",
        ">=1.0.0, <2.0.0",
        [">=1.0.0", "<2.0.0"],
        "00000000011111111000011000000000"
    ),
    req_case!(
        "> 0.0.9, <= 2.5.3",
        ">0.0.9, <=2.5.3",
        [">0.0.9", "<=2.5.3"],
        "00001111111111111101111100000000"
    ),
    // 合取顺序参与结构身份（交换顺序是不同的 VersionReq 结构）。
    req_case!(
        "<2.0.0, >=1.0.0",
        "<2.0.0, >=1.0.0",
        ["<2.0.0", ">=1.0.0"],
        "00000000011111111000011000000000"
    ),
    // wildcard 的不同拼写结构相同。
    req_case!("1.x", "1.*", ["1.*"], "00000000011111111000011000000000"),
    req_case!("1.*.*", "1.*", ["1.*"], "00000000011111111000011000000000"),
    req_case!(
        "0.0.*",
        "0.0.*",
        ["0.0.*"],
        "11110000000000000000000000000000"
    ),
    // prerelease gate：稳定版正常匹配；prerelease 只有 mmp 精确且带 pre 的
    // comparator 在场才可能放行。
    req_case!("^1", "^1", ["^1"], "00000000011111111000011000000000"),
    req_case!(
        ">=1.2.3-alpha, <1.3.0",
        ">=1.2.3-alpha, <1.3.0",
        [">=1.2.3-alpha", "<1.3.0"],
        "00000000000001110000000000001101"
    ),
    req_case!(
        "1.2.3-alpha",
        "^1.2.3-alpha",
        ["^1.2.3-alpha"],
        "00000000000001111000001000001101"
    ),
    // build 在每个 comparator 上 parse 时丢弃。
    req_case!(
        "=1.2.3+build",
        "=1.2.3",
        ["=1.2.3"],
        "00000000000001000000000000000000"
    ),
    // 缺省位写法。
    req_case!(
        ">=1.2",
        ">=1.2",
        [">=1.2"],
        "00000000000011111110001100000000"
    ),
    req_case!("<1", "<1", ["<1"], "11111111100000000001100000000000"),
    // 看似"区间"的连字符写法其实是 prerelease：parse 成功但语义完全不同。
    req_case!(
        "1.2.3-2.3.4",
        "^1.2.3-2.3.4",
        ["^1.2.3-2.3.4"],
        "00000000000001111000001000001101"
    ),
    // 零主版本 caret。
    req_case!("^0.2", "^0.2", ["^0.2"], "00000011000000000000000000000000"),
    // u64::MAX 主版本：覆盖超大整数在 req 侧的往返。
    req_case!(
        "=18446744073709551615.0.0",
        "=18446744073709551615.0.0",
        ["=18446744073709551615.0.0"],
        "00000000000000000010000000000000"
    ),
];

const REQ_ERRORS: &[(&str, &str)] = &[
    (
        "",
        "unexpected end of input while parsing major version number",
    ),
    (
        "\0",
        "unexpected character '\\0' while parsing major version number",
    ),
    (
        "\t>=1.0.0",
        "unexpected character '\\t' while parsing major version number",
    ),
    (
        ">=1.0.0\t",
        "expected comma after patch version number, found '\\t'",
    ),
    (
        ">= >= 0.0.2",
        "unexpected character '>' while parsing major version number",
    ),
    (
        ">== 0.0.2",
        "unexpected character '=' while parsing major version number",
    ),
    (
        "a.0.0",
        "unexpected character 'a' while parsing major version number",
    ),
    (
        "1.0.0-",
        "empty identifier segment in pre-release identifier",
    ),
    (
        ">=",
        "unexpected end of input while parsing major version number",
    ),
    (
        "1.0.0 2.0.0",
        "expected comma after patch version number, found '2'",
    ),
    (
        "1.2.3 - 2.3.4",
        "expected comma after patch version number, found '-'",
    ),
    (
        "=1.2.3 || =2.3.4",
        "expected comma after patch version number, found '|'",
    ),
    ("*.*", "unexpected character after wildcard in version req"),
    ("*.1", "unexpected character after wildcard in version req"),
    (
        "1.*.1",
        "unexpected character after wildcard in version req",
    ),
    (
        "*, 0.20.0-any",
        "wildcard req (*) must be the only comparator in the version req",
    ),
    (
        "0.20.0-any, *",
        "wildcard req (*) must be the only comparator in the version req",
    ),
    (
        "^1.0.0,",
        "unexpected end of input while parsing major version number",
    ),
    (
        ">18446744073709551616.0.0",
        "value of major version number exceeds u64::MAX",
    ),
];

#[test]
fn req_table() {
    let probes = probes();
    for case in REQ_CASES {
        let parsed = req(case.input);

        let actual_display = parsed.to_string();
        assert_eq!(
            actual_display,
            case.display.unwrap(),
            "Display mismatch for req input {:?}",
            case.input,
        );

        // comparator 序列身份：数量、顺序、逐项规范化 Display。
        assert_eq!(
            parsed.comparators.len(),
            case.comparators.len(),
            "comparator count mismatch for {:?}",
            case.input,
        );
        for (i, comparator) in parsed.comparators.iter().enumerate() {
            assert_eq!(
                comparator.to_string(),
                case.comparators[i],
                "comparator #{i} mismatch for req input {:?}",
                case.input,
            );
        }

        // 再解析：规范化文本必须回到同一个结构。
        let reparsed = req(&actual_display);
        assert_eq!(
            reparsed, parsed,
            "reparse mismatch for req input {:?}",
            case.input
        );
        assert_eq!(
            reparsed.to_string(),
            actual_display,
            "Display not idempotent for req input {:?}",
            case.input,
        );

        let actual_vector = match_vector(&parsed, &probes);
        assert_eq!(
            actual_vector,
            case.vector,
            "req match vector mismatch for {:?}\nprobes in order: {:?}",
            case.input,
            probes.iter().map(Version::to_string).collect::<Vec<_>>(),
        );
    }

    for (input, expected_error) in REQ_ERRORS {
        let err = VersionReq::parse(input).unwrap_err();
        assert_eq!(
            err.to_string(),
            *expected_error,
            "error mismatch for {input:?}"
        );
    }
}

// ==========================================================================
// build metadata 契约：参与结构相等/Hash/Display/Ord，但不参与版本优先级与匹配
//
// 这四件事分别断言，绝不用单个 assert_eq 代替：
//   (a) 结构相等：build 不同 => Version 不相等，Hash 也不同（高概率锁定）。
//   (b) Display：build 文本被完整保留并参与输出。
//   (c) 版本优先级 cmp_precedence：build 被完全忽略。
//   (d) matches：req / comparator 两侧都不看 build。
// ==========================================================================

#[test]
fn build_metadata_contract() {
    let plain = version("1.2.3");
    let build_a = version("1.2.3+aaa");
    let build_b = version("1.2.3+bbb");
    let pre_with_build = version("1.2.3-alpha+aaa");
    let pre_without_build = version("1.2.3-alpha");

    // (a) 结构相等与 Hash：build 是 Version 的身份字段。
    assert_ne!(plain, build_a, "build must participate in structural Eq");
    assert_ne!(build_a, build_b, "different build must be unequal");
    assert_ne!(
        pre_with_build, pre_without_build,
        "build must participate in structural Eq even when pre is present",
    );
    assert_ne!(
        hash64(&build_a),
        hash64(&build_b),
        "DefaultHasher collision on distinct builds (astronomically unlikely)",
    );

    // BuildMetadata 自身也按字符串内容相等/排序。
    assert_eq!(
        BuildMetadata::new("aaa").unwrap(),
        BuildMetadata::new("aaa").unwrap()
    );
    assert_ne!(
        BuildMetadata::new("aaa").unwrap(),
        BuildMetadata::new("bbb").unwrap()
    );

    // (b) Display：build 一定出现在输出里，且允许的前导零原样保留。
    assert_eq!(build_a.to_string(), "1.2.3+aaa");
    assert_eq!(version("1.2.3+007").to_string(), "1.2.3+007");
    assert_eq!(BuildMetadata::new("007").unwrap().to_string(), "007");

    // (c) 版本优先级：build 完全不参与。
    use std::cmp::Ordering;
    assert_eq!(plain.cmp_precedence(&build_a), Ordering::Equal);
    assert_eq!(build_a.cmp_precedence(&build_b), Ordering::Equal);
    assert_eq!(
        pre_with_build.cmp_precedence(&pre_without_build),
        Ordering::Equal,
    );

    // 但派生 Ord（结构总序）仍区分 build。
    assert!(build_a < build_b);

    // (d) matches：req 与单个 comparator 都忽略候选版本上的 build。
    let caret_req = req("^1.2.3");
    assert!(caret_req.matches(&build_a));
    assert!(caret_req.matches(&plain));
    let exact = comparator("=1.2.3");
    assert!(exact.matches(&build_a));
    assert!(exact.matches(&version("1.2.3+anything.007")));

    // 要求侧书写的 build 在 parse 阶段即被丢弃，连结构都留不下。
    let req_with_build = req("=1.2.3+ignored");
    assert_eq!(req_with_build, req("=1.2.3"));
    assert!(req_with_build.matches(&build_b));
}

#[test]
fn build_metadata_own_ordering_allows_leading_zero() {
    // 文档示例顺序：0 < 00 < 1 < 01 < 001 ...（仅 build 允许前导零）。
    let chain = ["0", "00", "1", "01", "001", "2", "1a0", "a"];
    let parsed: Vec<BuildMetadata> = chain
        .iter()
        .map(|s| BuildMetadata::new(s).unwrap())
        .collect();
    for window in parsed.windows(2) {
        assert!(
            window[0] < window[1],
            "{} should be < {}",
            window[0],
            window[1]
        );
    }
}

// ==========================================================================
// 身份字段：结构相等只比较解析后的字段，不比较原字符串
// ==========================================================================

#[test]
fn version_identity_fields() {
    // 同一结构的不同拼写：数值位置无前导零可改，所以这里主要验证 build/pre
    // 的字符串身份，以及 Eq/Hash/Ord 三者一致。
    let a = version("1.2.3-alpha+build");
    let b = {
        let mut built = Version::new(1, 2, 3);
        built.pre = Prerelease::new("alpha").unwrap();
        built.build = BuildMetadata::new("build").unwrap();
        built
    };
    assert_eq!(a, b);
    assert_eq!(hash64(&a), hash64(&b), "Eq 一致时 Hash 必须一致");

    // 五个字段各自都能单独制造不相等：major/minor/patch/pre/build。
    assert_ne!(version("1.2.3"), version("2.2.3"));
    assert_ne!(version("1.2.3"), version("1.3.3"));
    assert_ne!(version("1.2.3"), version("1.2.4"));
    assert_ne!(version("1.2.3-alpha"), version("1.2.3-beta"));
    assert_ne!(version("1.2.3+a"), version("1.2.3+b"));
}

#[test]
fn prerelease_identity_and_empty_constant() {
    // 空 prerelease 只能通过 Prerelease::new("") / EMPTY 得到；
    // Version 语法中 "-" 后必须非空。
    assert_eq!(Prerelease::new("").unwrap(), Prerelease::EMPTY);
    assert!(Prerelease::new("").unwrap().is_empty());
    assert!(Version::parse("1.0.0-").is_err());

    assert_eq!(
        Prerelease::new("a.b").unwrap(),
        Prerelease::new("a.b").unwrap()
    );
    assert_ne!(
        Prerelease::new("a.b").unwrap(),
        Prerelease::new("a.b.c").unwrap()
    );
    assert_eq!(
        hash64(&Prerelease::new("a.b").unwrap()),
        hash64(&Prerelease::new("a.b").unwrap())
    );
    // "-" 是合法的单段 identifier。
    assert_eq!(Prerelease::new("-").unwrap().as_str(), "-");
}

#[test]
fn req_identity_is_ordered_comparator_sequence() {
    // 相同 comparator 序列：不同原拼写（x/X/*、空白、caret 缺省）=> 相等。
    assert_eq!(req("1.x"), req("1.X"));
    assert_eq!(req("1.x"), req("1.*.*"));
    assert_eq!(req("  >= 1.0.0 , < 2.0.0 "), req(">=1.0.0,<2.0.0"));
    assert_eq!(hash64(&req("1.x")), hash64(&req("1.*")));

    // 合取的书写顺序不同 => 结构不相等，即使全域谓词相同（见有限见证测试）。
    let ab = req(">=1.0.0, <2.0.0");
    let ba = req("<2.0.0, >=1.0.0");
    assert_ne!(ab, ba);
    assert_ne!(ab.to_string(), ba.to_string());

    // 去重一个 comparator 即得到不同结构。
    assert_ne!(req(">=1.0.0"), req(">=1.0.0, <2.0.0"));
}

// ==========================================================================
// 有限见证：probe 向量相同 != 全域等价
// ==========================================================================

/// 在给定 probe 切片上比较两个 req 的匹配向量是否逐位相同。
#[track_caller]
fn assert_same_vector_on(a: &str, b: &str, probes: &[Version]) {
    let ra = req(a);
    let rb = req(b);
    let va = match_vector(&ra, probes);
    let vb = match_vector(&rb, probes);
    assert_eq!(
        va, vb,
        "witness setup failed: {a:?} vs {b:?} differ even on these probes"
    );
}

#[test]
fn finite_witness_same_vector_but_not_globally_equivalent() {
    // 星号与 ">=0.0.0-0"：在全部*稳定* probe 上向量完全一致，但对
    // 0.0.0-alpha 行为相反。这是"向量相同不等于全域等价"的有限见证。
    let stable_only: Vec<Version> = probes()
        .into_iter()
        .filter(|probe| probe.pre.is_empty())
        .collect();
    assert_same_vector_on("*", ">=0.0.0-0", &stable_only);

    // 反例 witness：扩大到 prerelease 后立刻区分。
    let star = VersionReq::STAR;
    let zero_pre = req(">=0.0.0-0");
    let alpha = version("0.0.0-alpha");
    assert!(!star.matches(&alpha), "* must not match any prerelease");
    assert!(zero_pre.matches(&alpha), ">=0.0.0-0 matches 0.0.0-alpha");

    // 第二个有限见证：~1.2.3 与 =1.2.3 在只含 1.2.3 / 1.2.3-alpha 时一致，
    // 但 1.2.4 立刻区分（~ 放行 patch 升级，= 不放行）。
    let narrow = [version("1.2.3"), version("1.2.3-alpha")];
    assert_same_vector_on("~1.2.3", "=1.2.3", &narrow);
    assert!(req("~1.2.3").matches(&version("1.2.4")));
    assert!(!req("=1.2.3").matches(&version("1.2.4")));
}

#[test]
fn same_predicate_does_not_mean_same_structure() {
    // 反向例子：全域谓词相同，但字符串往返与结构相等都不成立。
    // >=1.0.0,<2.0.0 与交换顺序版本在完整 probe 库上向量一致，且全域等价。
    let probes = probes();
    let ab = req(">=1.0.0, <2.0.0");
    let ba = req("<2.0.0, >=1.0.0");
    assert_eq!(match_vector(&ab, &probes), match_vector(&ba, &probes));
    assert_ne!(ab, ba, "commuted conjunctions are different structures");

    // 同理，caret/wildcard/exact 三种拼写全域谓词一致但规范化文本各异。
    let spellings = ["^1", "1.*", "=1", "~1"];
    for left in &spellings {
        for right in &spellings {
            assert_eq!(
                match_vector(&req(left), &probes),
                match_vector(&req(right), &probes)
            );
        }
    }
    assert_eq!(req("^1").to_string(), "^1");
    assert_eq!(req("1.*").to_string(), "1.*");
    assert_eq!(req("=1").to_string(), "=1");
    assert_eq!(req("~1").to_string(), "~1");
}

// ==========================================================================
// 独立反例：每类陷阱各自一个 self-contained 测试
// ==========================================================================

/// 反例 1：prerelease identifier 的数值排序与字母排序方向相反。
#[test]
fn counterexample_prerelease_numeric_vs_alpha() {
    // 点分隔的纯数字段按数值：2 < 11。
    let p2 = Prerelease::new("pre.2").unwrap();
    let p11 = Prerelease::new("pre.11").unwrap();
    assert!(
        p2 < p11,
        "numeric identifiers compare numerically: {p2} < {p11}"
    );

    // 未用点分开、整段含字母时按 ASCII：pre11 < pre2（'1' < '2'）。
    let pa2 = Prerelease::new("pre2").unwrap();
    let pa11 = Prerelease::new("pre11").unwrap();
    assert!(
        pa11 < pa2,
        "alphanumeric identifiers compare in ASCII: {pa11} < {pa2}"
    );

    // 同样的方向性差异必须体现在 Version 排序上。
    assert!(version("1.0.0-pre.2") < version("1.0.0-pre.11"));
    assert!(version("1.0.0-pre11") < version("1.0.0-pre2"));

    // 纯数字段恒小于含字母段：1 < 1a0 < a。
    assert!(Prerelease::new("1").unwrap() < Prerelease::new("1a0").unwrap());
    assert!(Prerelease::new("1a0").unwrap() < Prerelease::new("a").unwrap());
}

/// 反例 2：零主版本 caret 与普通 caret 的兼容范围完全不同。
#[test]
fn counterexample_zero_major_caret() {
    // ^1 允许 minor/patch 升级；^0 只允许 0.x；^0.2 只允许 0.2.x。
    assert!(req("^1").matches(&version("1.9.0")));
    assert!(!req("^1").matches(&version("2.0.0")));
    assert!(!req("^0").matches(&version("1.0.0")));
    assert!(req("^0").matches(&version("0.9.0")));

    // 关键不对称：^1.2 放行 1.3.0，^0.2 不放行 0.3.0。
    assert!(req("^1.2").matches(&version("1.3.0")));
    assert!(!req("^0.2").matches(&version("0.3.0")));
    assert!(req("^0.2").matches(&version("0.2.9")));

    // ^0.0 再收紧一档：只放行 0.0.x。
    assert!(req("^0.0").matches(&version("0.0.9")));
    assert!(!req("^0.0").matches(&version("0.1.0")));
}

/// 反例 3：比较器缺失 minor/patch 时的隐式区间，与"看起来像的完整写法"不同。
#[test]
fn counterexample_missing_components() {
    // =1 等价于 >=1.0.0, <2.0.0，而不是只匹配 1.0.0。
    assert!(comparator("=1").matches(&version("1.5.7")));
    assert!(!comparator("=1").matches(&version("2.0.0")));
    assert!(!comparator("=1.0.0").matches(&version("1.5.7")));

    // 注意 matches 路径与 lib 文档里的"区间等价"并不逐版本同构：
    // <1.2 的 minor 已给定、patch 缺失，比较器在 minor 相等处立即返回 false，
    // 因此连 1.2.0-alpha（precedence 低于 1.2.0）都不放行；它只放行 minor
    // 严格更小的版本（prerelease gate 另算）。
    let less = comparator("<1.2");
    assert!(less.matches(&version("1.1.9")));
    assert!(!less.matches(&version("1.2.0")));
    assert!(!less.matches(&version("1.2.0-alpha")));

    // 对称地 >1.2 在 minor 相等、patch 缺失处立即返回 false：1.2.5 不满足；
    // 1.3.0 满足。
    let greater = comparator(">1.2");
    assert!(!greater.matches(&version("1.2.5")));
    assert!(greater.matches(&version("1.3.0")));
}

/// 反例 4：显式 operator 与 wildcard 连用时，"*" 被解析为缺省位并在
/// Display 中消失，例如 `=1.*` 规范化为 `=1`；它与 default-op 的 `1.*`
/// （规范化为 `1.*`）结构 op 不同。
#[test]
fn counterexample_wildcard_with_explicit_op() {
    let default_op = comparator("1.*");
    let explicit_eq = comparator("=1.*");
    assert_eq!(default_op.op, Op::Wildcard);
    assert_eq!(explicit_eq.op, Op::Exact);
    assert_eq!(default_op.to_string(), "1.*");
    assert_eq!(explicit_eq.to_string(), "=1");
    // 两者在本实现里的匹配集合一致——但这是谓词层面的巧合，结构并不相等。
    assert_ne!(default_op, explicit_eq);

    // 通配符不能出现在 major 之外后还带数字：*.1 / 1.*.1 都拒绝（锁定接受集）。
    assert!(Comparator::parse("*.1").is_err());
    assert!(Comparator::parse("1.*.1").is_err());
}

/// 连字符陷阱：`1.2.3 - 2.3.4` 不是 npm 风格区间（缺少逗号直接拒绝），
/// 而 `1.2.3-2.3.4` 能解析，却被当成 prerelease，谓词与区间天差地别。
#[test]
fn counterexample_hyphen_is_not_a_range() {
    assert!(VersionReq::parse("1.2.3 - 2.3.4").is_err());
    let misread = req("1.2.3-2.3.4");
    assert_eq!(misread.comparators.len(), 1);
    assert_eq!(misread.comparators[0].pre.as_str(), "2.3.4");
    assert!(!misread.matches(&version("2.0.0")));
    assert!(misread.matches(&version("1.2.3-2.3.4")));
}

/// 比较器数量上限：最多 32 个（深度 0..=31），第 33 个直接报错。
#[test]
fn comparator_count_limit_is_32() {
    let thirty_two = (0..32)
        .map(|i| format!(">={i}.0.0"))
        .collect::<Vec<_>>()
        .join(", ");
    let parsed = req(&thirty_two);
    assert_eq!(parsed.comparators.len(), 32);
    // 往返保留全部 32 个。
    assert_eq!(req(&parsed.to_string()).comparators.len(), 32);

    let thirty_three = (0..33)
        .map(|i| format!(">={i}.0.0"))
        .collect::<Vec<_>>()
        .join(", ");
    let err = VersionReq::parse(&thirty_three).unwrap_err();
    assert_eq!(err.to_string(), "excessive number of version comparators");
}

/// 空白规范化：仅 ASCII 空格被容忍；tab/内部空白拒绝，且 Display 统一重排。
#[test]
fn whitespace_is_normalized_space_only() {
    assert_eq!(req("  >=1.0.0 ,  <2.0.0 ").to_string(), ">=1.0.0, <2.0.0");
    // 结尾空格会被吃掉（最后一个 comparator 的尾部空白）。
    assert_eq!(req("1.0.0 ").to_string(), "^1.0.0");
    // tab 不被当作空白。
    assert!(VersionReq::parse("\t1.0.0").is_err());
    assert!(VersionReq::parse("1.0.0\t").is_err());
    // 逗号缺失（只用空格分隔）拒绝。
    assert!(VersionReq::parse(">=1.0.0 <2.0.0").is_err());
    // Version 语法完全不容忍空白。
    assert!(Version::parse(" 1.0.0").is_err());
    assert!(Version::parse("1.0.0 ").is_err());
}
