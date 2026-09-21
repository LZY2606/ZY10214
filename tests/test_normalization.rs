//! 规范化边界的表驱动回归测试。
//!
//! 本文件刻意把下面三个概念分开断言，不允许用一个 `assert_eq!` 混为一谈：
//!
//! 1. 字符串往返（parse 成功后的 `Display` 文本，以及二次 parse 的结构）；
//! 2. 结构相等 / 全序 / Hash（`Eq`、`Ord`、`Hash` 各自看哪些字段）；
//! 3. 匹配等价（对一组 probe versions 的 `matches` 向量）。
//!
//! 两条 `VersionReq` 在某组 probe 上向量相同，只是“有限见证”，并不构成
//! 全域等价；`finite_witness_*` 测试同时给出区分二者的 probe 反例。
//!
//! 背景说明见仓库根目录的 `NORMALIZATION.md`。

mod util;

use crate::util::*;
use semver::{Comparator, Op, Prerelease, Version, VersionReq};
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const PROBES: &[&str] = &[
    "0.0.0",
    "0.0.1",
    "0.1.0",
    "0.9.0",
    "1.0.0-alpha",
    "1.0.0-alpha.1",
    "1.0.0",
    "1.0.0+sha",
    "1.0.1",
    "1.0.1+meta",
    "1.1.0",
    "1.2.0-pre",
    "2.0.0-alpha",
    "2.0.0",
];

#[track_caller]
fn hash64<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[track_caller]
fn matches_vector(req: &VersionReq, probes: &[&str]) -> Vec<bool> {
    probes
        .iter()
        .map(|text| req.matches(&version(text)))
        .collect()
}

#[track_caller]
fn bool_vector(letters: &str) -> Vec<bool> {
    letters
        .chars()
        .map(|ch| {
            assert!(ch == 'T' || ch == 'f', "向量只能由 T/f 组成: {letters}");
            ch == 'T'
        })
        .collect()
}

#[track_caller]
fn assert_vector(req_text: &str, req: &VersionReq, expected: &str) {
    let actual = matches_vector(req, PROBES);
    let expected = bool_vector(expected);
    assert_eq!(
        expected.len(),
        PROBES.len(),
        "用例向量长度必须与 PROBES 一致"
    );
    assert_eq!(
        actual, expected,
        "req `{req_text}` 的 probe 匹配向量不匹配；probes = {PROBES:?}",
    );
}

struct VersionCase {
    input: &'static str,
    display: &'static str,
    major: u64,
    minor: u64,
    patch: u64,
    pre: &'static str,
    build: &'static str,
}

const VERSION_CASES: &[VersionCase] = &[
    VersionCase {
        input: "1.2.3",
        display: "1.2.3",
        major: 1,
        minor: 2,
        patch: 3,
        pre: "",
        build: "",
    },
    VersionCase {
        input: "1.2.3-alpha.1",
        display: "1.2.3-alpha.1",
        major: 1,
        minor: 2,
        patch: 3,
        pre: "alpha.1",
        build: "",
    },
    VersionCase {
        input: "1.2.3+build.42",
        display: "1.2.3+build.42",
        major: 1,
        minor: 2,
        patch: 3,
        pre: "",
        build: "build.42",
    },
    VersionCase {
        input: "1.2.3-alpha1+build5",
        display: "1.2.3-alpha1+build5",
        major: 1,
        minor: 2,
        patch: 3,
        pre: "alpha1",
        build: "build5",
    },
    // build metadata 允许前导零；前导零被原样保留，不会被规范化掉。
    VersionCase {
        input: "1.2.3+007",
        display: "1.2.3+007",
        major: 1,
        minor: 2,
        patch: 3,
        pre: "",
        build: "007",
    },
    VersionCase {
        input: "0.4.0-beta.1+0851523",
        display: "0.4.0-beta.1+0851523",
        major: 0,
        minor: 4,
        patch: 0,
        pre: "beta.1",
        build: "0851523",
    },
    // u64::MAX 边界：可以解析且 Display 后往返不变。
    VersionCase {
        input: "18446744073709551615.0.0",
        display: "18446744073709551615.0.0",
        major: u64::MAX,
        minor: 0,
        patch: 0,
        pre: "",
        build: "",
    },
    // 含字母的 prerelease 段允许以 0 开头（它不是纯数字段）。
    VersionCase {
        input: "1.2.3-0a.1",
        display: "1.2.3-0a.1",
        major: 1,
        minor: 2,
        patch: 3,
        pre: "0a.1",
        build: "",
    },
];

#[test]
fn version_table_parse_display_reparse() {
    for case in VERSION_CASES {
        let parsed = version(case.input);

        // 身份字段（Eq/Hash 看的字段）。
        let expected_struct = Version {
            major: case.major,
            minor: case.minor,
            patch: case.patch,
            pre: prerelease(case.pre),
            build: build_metadata(case.build),
        };
        assert_eq!(parsed, expected_struct, "parse 结构不符: `{}`", case.input);

        // Display 是一条独立断言：记录哪些拼写被保留、哪些会规范化。
        assert_to_string(&parsed, case.display);

        // 二次 parse 必须得到结构相同的值（Version 的 Display 对合法输入是往返保持的）。
        let reparsed = version(&parsed.to_string());
        assert_eq!(
            reparsed, parsed,
            "Display -> parse 往返不保持结构: `{}` -> `{}`",
            case.input, case.display,
        );
    }
}

const VERSION_ERRORS: &[(&str, &str)] = &[
    ("", "empty string, expected a semver version"),
    (
        " 1.0.0",
        "unexpected character ' ' while parsing major version number",
    ),
    (
        "1.0.0 ",
        "unexpected character ' ' after patch version number",
    ),
    (
        "1.0.0\t",
        "unexpected character '\\t' after patch version number",
    ),
    (
        "1",
        "unexpected end of input while parsing major version number",
    ),
    (
        "1.2",
        "unexpected end of input while parsing minor version number",
    ),
    (
        "1.2.3.0",
        "unexpected character '.' after patch version number",
    ),
    ("01.0.0", "invalid leading zero in major version number"),
    ("1.02.0", "invalid leading zero in minor version number"),
    ("1.2.03", "invalid leading zero in patch version number"),
    ("1.0.0-01", "invalid leading zero in pre-release identifier"),
    // build metadata 是唯一允许前导零的位置，与 prerelease 形成独立反例。
    (
        "1.0.0-",
        "empty identifier segment in pre-release identifier",
    ),
    ("1.0.0+", "empty identifier segment in build metadata"),
    (
        "1.0.0-alpha..1",
        "empty identifier segment in pre-release identifier",
    ),
    ("1.0.0+x@y", "unexpected character '@' after build metadata"),
    ("1.0.0+@x", "empty identifier segment in build metadata"),
    (
        "1.0.0+build_1",
        "unexpected character '_' after build metadata",
    ),
    (
        "1.0.0-alpha_1",
        "unexpected character '_' after pre-release identifier",
    ),
    (
        "v1.0.0",
        "unexpected character 'v' while parsing major version number",
    ),
    (
        "18446744073709551616.0.0",
        "value of major version number exceeds u64::MAX",
    ),
    (
        "1.111111111111111111111.0",
        "value of minor version number exceeds u64::MAX",
    ),
    (
        "0.0.18446744073709551616",
        "value of patch version number exceeds u64::MAX",
    ),
];

#[test]
fn version_table_parse_failures() {
    for (input, message) in VERSION_ERRORS {
        assert_to_string(version_err(input), message);
        assert!(
            Version::parse(input).is_err(),
            "失败用例二次确认仍应失败: `{input}`",
        );
    }
}

/// build metadata 的实际契约：参与 Version 的结构相等、全序与显示，
/// 但不参与版本优先级（cmp_precedence），也不参与任何 matches 判断。
#[test]
fn build_metadata_equality_vs_precedence_vs_matches() {
    let plain = version("1.20.0");
    let sha_a = version("1.20.0+bc17664");
    let sha_b = version("1.20.0+c144a98");

    // 1) 结构相等：build 不同 => Version 不相等（不能只断言一个方向）。
    assert_ne!(plain, sha_a, "无 build 与有 build 不应结构相等");
    assert_ne!(sha_a, sha_b, "不同 build 不应结构相等");
    assert_eq!(sha_a, version("1.20.0+bc17664"), "相同 build 应结构相等");

    // 2) Hash 与 Eq 一致：build 参与 Hash，不同 build 可能且应给出不同桶位。
    assert_ne!(
        hash64(&sha_a),
        hash64(&sha_b),
        "不同 build 的 Hash 应可区分"
    );
    assert_eq!(hash64(&sha_a), hash64(&version("1.20.0+bc17664")));

    // 3) 优先级：build 完全不参与；cmp_precedence 判等，但 Ord（结构全序）不判等。
    assert_eq!(
        plain.cmp_precedence(&sha_a),
        Ordering::Equal,
        "build 不应影响 cmp_precedence",
    );
    assert_eq!(
        sha_a.cmp_precedence(&sha_b),
        Ordering::Equal,
        "不同 build 的 cmp_precedence 应相等",
    );
    assert_ne!(
        plain.cmp(&sha_a),
        Ordering::Equal,
        "Ord 会比较 build，不应相等"
    );
    assert_ne!(sha_a.cmp(&sha_b), Ordering::Equal, "Ord 应区分不同 build");

    // 4) BuildMetadata 自身的全序：build 允许前导零，数值段按数值比较，
    //    再用原始长度（前导零）打破平局：0 < 00 < 1 < 01。
    let chain = ["0", "00", "1", "01", "001", "2", "10", "a"];
    for window in chain.windows(2) {
        let left = build_metadata(window[0]);
        let right = build_metadata(window[1]);
        assert!(
            left < right,
            "build `{}` 应小于 build `{}`",
            window[0],
            window[1]
        );
    }

    // 5) matches 忽略 build：同一个 req 对带任意 build 的 1.20.0 结论一致。
    let req = req("^1.20.0");
    for probe in [
        "1.20.0",
        "1.20.0+bc17664",
        "1.20.0+c144a98",
        "1.20.0-x+build",
    ] {
        let parsed = version(probe);
        let matched = req.matches(&parsed);
        // build 不改变结论；前三个无 prerelease 必匹配，最后一个受 prerelease 规则约束。
        assert_eq!(
            matched,
            !probe.contains("-x"),
            "build 不应改变 matches 结果: `{probe}`",
        );
    }
}

/// prerelease identifier：数值段按数值、字母段按 ASCII、数值段总小于非数值段，
/// 以及“字段数更多者更大”。alpha11 < alpha2 是与直觉相反的独立反例。
#[test]
fn prerelease_identifier_ordering_counterexamples() {
    // 数值段：8 < 12（按数值而非 ASCII）。
    assert!(prerelease("pre.8") < prerelease("pre.12"));
    // 字母段：ASCII 字典序，pre12 < pre8（因为 '1' < '8'）。
    assert!(prerelease("pre12") < prerelease("pre8"));
    assert!(version("1.0.0-pre12") < version("1.0.0-pre8"));
    // 任何纯数值段小于任何含字母/连字符的段。
    assert!(prerelease("alpha.1") < prerelease("alpha.a"));
    assert!(prerelease("1") < prerelease("alpha"));
    // 连字符是单段内的合法字符；含连字符的整段与 ASCII 排序一致
    //（'-' 为 0x2D，故 "a-b" < "a.b" 这种逐字符比较由分段规则决定）。
    assert_eq!(prerelease("beta-1").as_str(), "beta-1");
    assert!(prerelease("1-") < prerelease("1a"));
    // "a-b" 是单段，"a.c" 第一段为更短的 "a"（公共前缀 => 更短者更小）。
    assert!(prerelease("a.c") < prerelease("a-b"));
    // 字段更多且前缀相等时更大；空 prerelease（正式版）最大。
    assert!(prerelease("alpha") < prerelease("alpha.1"));
    assert!(prerelease("alpha.1") < Prerelease::EMPTY);
    assert_eq!(
        prerelease("alpha.1").cmp(&prerelease("alpha.1")),
        Ordering::Equal
    );
    // 前导零在 prerelease 中直接拒绝，在 build 中接受（解析接受集不变）。
    assert!(Prerelease::new("01").is_err());
    assert_eq!(build_metadata("01").as_str(), "01");

    // 这些差异最终体现在 Version 的 Ord 上（cmp_precedence 也看 pre）。
    assert!(version("1.0.0-alpha") < version("1.0.0-alpha.1"));
    assert!(version("1.0.0-alpha.1") < version("1.0.0-alpha.beta"));
    assert!(version("1.0.0-rc.1") < version("1.0.0"));
}

struct OrdPair {
    less: &'static str,
    greater: &'static str,
    note: &'static str,
}

const VERSION_ORDER: &[OrdPair] = &[
    OrdPair {
        less: "1.0.0-alpha",
        greater: "1.0.0",
        note: "prerelease 小于正式版",
    },
    OrdPair {
        less: "1.5.0",
        greater: "1.19.0",
        note: "minor 按数值比较",
    },
    OrdPair {
        less: "1.0.0-pre.8",
        greater: "1.0.0-pre.12",
        note: "数值 prerelease 段",
    },
    OrdPair {
        less: "1.0.0-pre12",
        greater: "1.0.0-pre8",
        note: "字母 prerelease 段按 ASCII",
    },
    OrdPair {
        less: "1.20.0",
        greater: "1.20.0+1",
        note: "无 build 在 Ord 中最小（build 参与 Ord）",
    },
    OrdPair {
        less: "1.20.0+1",
        greater: "1.20.0+01",
        note: "build 数值段平局后按原始长度",
    },
];

#[test]
fn version_table_ordering() {
    for pair in VERSION_ORDER {
        let less = version(pair.less);
        let greater = version(pair.greater);
        assert!(
            less < greater,
            "{}（`{}` < `{}`）",
            pair.note,
            pair.less,
            pair.greater
        );
        assert_eq!(less.cmp(&greater), Ordering::Less);
        assert_eq!(greater.cmp(&less), Ordering::Greater);
        assert_ne!(less, greater, "排序不同的版本不应结构相等: {}", pair.note);
    }

    // 只在 build 上不同的两个版本，Ord 不相等但优先级相等。
    let a = version("1.0.0+1");
    let b = version("1.0.0+2");
    assert_ne!(a.cmp(&b), Ordering::Equal);
    assert_eq!(a.cmp_precedence(&b), Ordering::Equal);
}

struct ReqCase {
    input: &'static str,
    display: &'static str,
    vector: &'static str,
    comparators: &'static [ComparatorShape<'static>],
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct ComparatorShape<'a> {
    op: Op,
    major: u64,
    minor: Option<u64>,
    patch: Option<u64>,
    pre: &'a str,
}

fn shape(comparator: &Comparator) -> ComparatorShape<'_> {
    ComparatorShape {
        op: comparator.op,
        major: comparator.major,
        minor: comparator.minor,
        patch: comparator.patch,
        pre: comparator.pre.as_str(),
    }
}

fn assert_shape_eq(left: &[ComparatorShape<'_>], right: &[ComparatorShape<'_>], context: &str) {
    assert_eq!(left.len(), right.len(), "comparator 数量不符: {context}");
    for (i, (left, right)) in left.iter().zip(right).enumerate() {
        assert_eq!(left, right, "第 {i} 个 comparator 结构不符: {context}");
    }
}

const fn cmp(
    op: Op,
    major: u64,
    minor: Option<u64>,
    patch: Option<u64>,
    pre: &str,
) -> ComparatorShape<'_> {
    ComparatorShape {
        op,
        major,
        minor,
        patch,
        pre,
    }
}

const REQ_CASES: &[ReqCase] = &[
    // 缺省操作符规范化为 caret。
    ReqCase {
        input: "1.0.0",
        display: "^1.0.0",
        vector: "ffffffTTTTTfff",
        comparators: &[cmp(Op::Caret, 1, Some(0), Some(0), "")],
    },
    // 缺省 patch：结构里 None，Display 也不会补全；但匹配向量与 ^1.0.0 相同。
    ReqCase {
        input: "1.0",
        display: "^1.0",
        vector: "ffffffTTTTTfff",
        comparators: &[cmp(Op::Caret, 1, Some(0), None, "")],
    },
    // 缺省 minor/patch。
    ReqCase {
        input: "1",
        display: "^1",
        vector: "ffffffTTTTTfff",
        comparators: &[cmp(Op::Caret, 1, None, None, "")],
    },
    // wildcard：x/X/* 与缺省位归一成 Op::Wildcard，Display 规范成 `1.*`。
    ReqCase {
        input: "1.x",
        display: "1.*",
        vector: "ffffffTTTTTfff",
        comparators: &[cmp(Op::Wildcard, 1, None, None, "")],
    },
    ReqCase {
        // PROBES 中没有 1.2.x 稳定版（只有 1.2.0-pre，受 prerelease 门槛拒绝），
        // 因此这个用例在本语料上的向量全 false，是有意的空见证。
        input: "1.2.X",
        display: "1.2.*",
        vector: "ffffffffffffff",
        comparators: &[cmp(Op::Wildcard, 1, Some(2), None, "")],
    },
    // 全 wildcard 规范化为空 comparator 列表（STAR），Display 为 `*`。
    ReqCase {
        input: "X",
        display: "*",
        vector: "TTTTffTTTTTffT",
        comparators: &[],
    },
    ReqCase {
        input: "*",
        display: "*",
        vector: "TTTTffTTTTTffT",
        comparators: &[],
    },
    // 显式操作符 + wildcard：结构保留 Exact op，且缺省位的 `.*` 在 Display 中消失。
    // 这是先前未在文档中说明的拼写归并：`=1.*` 与 `=1` Display 完全相同。
    ReqCase {
        input: "=1.*",
        display: "=1",
        vector: "ffffffTTTTTfff",
        comparators: &[cmp(Op::Exact, 1, None, None, "")],
    },
    // comparator 上的 build metadata 被解析但完全丢弃（结构里没有 build 字段）。
    ReqCase {
        input: ">=1.2.3+meta",
        display: ">=1.2.3",
        vector: "fffffffffffffT",
        comparators: &[cmp(Op::GreaterEq, 1, Some(2), Some(3), "")],
    },
    ReqCase {
        input: "^1.2.3-alpha+build",
        display: "^1.2.3-alpha",
        vector: "ffffffffffffff",
        comparators: &[cmp(Op::Caret, 1, Some(2), Some(3), "alpha")],
    },
    // 操作符前后空白、逗号前后多重空白被规范化；结尾允许 ASCII 空格。
    ReqCase {
        input: "  >=  1.0.0  , < 2.0.0 ",
        display: ">=1.0.0, <2.0.0",
        vector: "ffffffTTTTTfff",
        comparators: &[
            cmp(Op::GreaterEq, 1, Some(0), Some(0), ""),
            cmp(Op::Less, 2, Some(0), Some(0), ""),
        ],
    },
    ReqCase {
        input: ">1.0.0-alpha, <1.0.0",
        display: ">1.0.0-alpha, <1.0.0",
        vector: "fffffTffffffff",
        comparators: &[
            cmp(Op::Greater, 1, Some(0), Some(0), "alpha"),
            cmp(Op::Less, 1, Some(0), Some(0), ""),
        ],
    },
    // 零主版本 caret 的三级收窄。
    ReqCase {
        input: "^0",
        display: "^0",
        vector: "TTTTffffffffff",
        comparators: &[cmp(Op::Caret, 0, None, None, "")],
    },
    ReqCase {
        input: "^0.0",
        display: "^0.0",
        vector: "TTffffffffffff",
        comparators: &[cmp(Op::Caret, 0, Some(0), None, "")],
    },
    ReqCase {
        input: "^0.0.0",
        display: "^0.0.0",
        vector: "Tfffffffffffff",
        comparators: &[cmp(Op::Caret, 0, Some(0), Some(0), "")],
    },
];

#[test]
fn req_table_parse_display_reparse_and_vector() {
    for case in REQ_CASES {
        let parsed = req(case.input);

        // 结构（身份字段）：逐个 comparator 的 op/major/minor/patch/pre。
        let actual_shapes: Vec<ComparatorShape<'_>> =
            parsed.comparators.iter().map(shape).collect();
        assert_shape_eq(&actual_shapes, case.comparators, case.input);

        // Display 规范化文本。
        assert_to_string(&parsed, case.display);

        // 二次 parse：规范化文本必须 parse 回结构相同的 req。
        let reparsed = req(&parsed.to_string());
        assert_eq!(
            reparsed, parsed,
            "req `{}` Display 后结构往返失败",
            case.input
        );
        let reparsed_shapes: Vec<ComparatorShape<'_>> =
            reparsed.comparators.iter().map(shape).collect();
        assert_shape_eq(&reparsed_shapes, case.comparators, case.input);

        // 匹配向量（独立于结构相等的第三件事）。
        assert_vector(case.input, &parsed, case.vector);
    }
}

const REQ_ERRORS: &[(&str, &str)] = &[
    (
        "",
        "unexpected end of input while parsing major version number",
    ),
    (
        "   ",
        "unexpected end of input while parsing major version number",
    ),
    (
        "\t^1.0.0",
        "unexpected character '\\t' while parsing major version number",
    ),
    (
        "^1.0.0\t",
        "expected comma after patch version number, found '\\t'",
    ),
    (
        "1.0 1.0",
        "expected comma after minor version number, found '1'",
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
        "*, 1.0.0",
        "wildcard req (*) must be the only comparator in the version req",
    ),
    (
        "@1.0.0",
        "unexpected character '@' while parsing major version number",
    ),
    (
        "1.0.0-",
        "empty identifier segment in pre-release identifier",
    ),
    ("1.0.0-01", "invalid leading zero in pre-release identifier"),
    (
        "1.0.0+build_1",
        "expected comma after build metadata, found '_'",
    ),
    (
        ">a.b",
        "unexpected character 'a' while parsing major version number",
    ),
    (
        ">=",
        "unexpected end of input while parsing major version number",
    ),
];

#[test]
fn req_table_parse_failures() {
    for (input, message) in REQ_ERRORS {
        assert_to_string(req_err(input), message);
        assert!(
            VersionReq::parse(input).is_err(),
            "失败用例二次确认: `{input}`"
        );
    }
}

const COMPARATOR_CASES: &[ReqCase] = &[
    // Comparator::parse 只接受单个 comparator；缺省 op 仍是 caret。
    ReqCase {
        input: "1.2.3-alpha",
        display: "^1.2.3-alpha",
        vector: "ffffffffffffff",
        comparators: &[cmp(Op::Caret, 1, Some(2), Some(3), "alpha")],
    },
    ReqCase {
        input: "  2.X  ",
        display: "2.*",
        vector: "fffffffffffffT",
        comparators: &[cmp(Op::Wildcard, 2, None, None, "")],
    },
    ReqCase {
        input: "2",
        display: "^2",
        vector: "fffffffffffffT",
        comparators: &[cmp(Op::Caret, 2, None, None, "")],
    },
    ReqCase {
        input: "=1.*.*",
        display: "=1",
        vector: "ffffffTTTTTfff",
        comparators: &[cmp(Op::Exact, 1, None, None, "")],
    },
];

#[test]
fn comparator_table() {
    for case in COMPARATOR_CASES {
        let parsed = comparator(case.input);
        let expected = case
            .comparators
            .first()
            .expect("comparator 用例需有一个形状");
        assert_eq!(
            shape(&parsed),
            *expected,
            "comparator 结构不符: `{}`",
            case.input
        );
        assert_to_string(&parsed, case.display);

        // 二次 parse 往返。
        let reparsed = comparator(&parsed.to_string());
        assert_eq!(reparsed, parsed, "comparator `{}` 往返失败", case.input);

        // Comparator::matches 自带 Cargo prerelease 门槛；把它包成单元素 req，
        // 其向量与 req 级评估一致（单 comparator 时二者规则相同）。
        let wrapped = VersionReq {
            comparators: vec![reparsed.clone()],
        };
        assert_vector(case.input, &wrapped, case.vector);
    }

    // comparator 上的 build 被解析接受并丢弃，随后它按 build 无关的方式匹配。
    let with_build = comparator("=1.0.0+ignored");
    assert_eq!(with_build, comparator("=1.0.0"));
    assert!(with_build.matches(&version("1.0.0+anything")));
    assert!(!with_build.matches(&version("1.0.1")));

    // 多 comparator 不是 Comparator::parse 的接受集。
    assert_to_string(
        comparator_err("1.2.3, 2.0.0"),
        "unexpected character ',' after patch version number",
    );
    assert_to_string(
        comparator_err("1.2.3+4."),
        "empty identifier segment in build metadata",
    );
}

/// 有限见证：不同字符串、不同结构的 VersionReq 在 PROBES 上向量暂时相同，
/// 但存在区分它们的 probe。这里显式给出反例，避免把有限见证误读为全域等价。
#[test]
fn finite_witness_different_string_not_universal() {
    // 见证一：`^1.0.0`（= Op::Caret 全精度）与 `1.0.*`（= Op::Wildcard 缺 patch）。
    let caret = req("^1.0.0");
    let wildcard = req("1.0.*");
    assert_ne!(
        caret, wildcard,
        "二者结构不同：Caret+全精度 vs Wildcard+缺 patch"
    );
    // PROBES 中 1.1.0 已能区分二者，因此先在一组不含 1.1.0 的有限 probe 上见证相同。
    let limited: Vec<&str> = PROBES
        .iter()
        .copied()
        .filter(|probe| *probe != "1.1.0")
        .collect();
    assert_eq!(
        matches_vector(&caret, &limited),
        matches_vector(&wildcard, &limited),
        "在去掉 1.1.0 的有限 probe 集上应暂时相同（有限见证）",
    );
    // 区分反例：caret 允许 minor 提升，wildcard 锁定 minor==0。
    let distinguishing = version("1.1.0");
    assert!(caret.matches(&distinguishing), "^1.0.0 应匹配 1.1.0");
    assert!(!wildcard.matches(&distinguishing), "1.0.* 不应匹配 1.1.0");

    // 见证二：`*` 与 `>=0.0.0` 在非 prerelease probe 上完全一致，
    // 但这不是因为字符串/结构等价——STAR 是空 comparator 列表。
    let star = VersionReq::STAR;
    let gte = req(">=0.0.0");
    assert_ne!(star, gte, "STAR（空列表）与 >=0.0.0 结构不同");
    assert_eq!(matches_vector(&star, PROBES), matches_vector(&gte, PROBES));
    // 该组 PROBES 已包含两者都拒绝的 prerelease（如 1.0.0-alpha），
    // 记录“* 不匹配任何 prerelease”这一与直觉相反的契约。
    assert!(!star.matches(&version("1.0.0-alpha")));
    assert!(!gte.matches(&version("1.0.0-alpha")));
    // 二者在本 crate 语义下全域一致；向量相同仅为有限见证，等价性另由文档承载。

    // 见证三：显式 prerelease 的 req 与 STAR 在非 prerelease 上大面积一致，
    // 但在同 m.m.p 的 prerelease probe 上分叉。
    let gated = req(">=1.0.0-alpha");
    assert_eq!(
        gated.matches(&version("1.5.0")),
        star.matches(&version("1.5.0"))
    );
    assert!(gated.matches(&version("1.0.0-alpha")));
    assert!(!star.matches(&version("1.0.0-alpha")));
}

/// 零主版本 caret：非零位右侧才可提升，三级各自收窄，独立反例。
#[test]
fn zero_major_caret_counterexamples() {
    // ^0.2.3 锁定 minor==2，只允许 patch 提升。
    let r = req("^0.2.3");
    assert!(r.matches(&version("0.2.3")));
    assert!(r.matches(&version("0.2.9")));
    assert!(!r.matches(&version("0.3.0")), "^0.2.3 不允许 minor 提升");
    assert!(!r.matches(&version("0.2.2")));
    assert!(!r.matches(&version("1.0.0")));

    // ^0.0.3 连 patch 都锁定（首非零位就是 patch）。
    let r = req("^0.0.3");
    assert!(r.matches(&version("0.0.3")));
    assert!(!r.matches(&version("0.0.4")), "^0.0.3 不允许 patch 提升");
    assert!(!r.matches(&version("0.1.0")));

    // ^0 与 ^0.0 的缺省位差异：前者允许任意 minor，后者锁定 minor==0。
    assert!(req("^0").matches(&version("0.9.0")));
    assert!(!req("^0.0").matches(&version("0.9.0")));
    assert!(req("^0.0").matches(&version("0.0.9")));
}

/// 比较器缺失位：minor/patch 为 None 在结构、Display 和边界比较中的独立反例。
#[test]
fn missing_components_counterexamples() {
    // >1.0 等价 >=1.1.0：缺失 patch 的“大于”以 minor 进位处理。
    assert!(!req(">1.0").matches(&version("1.0.5")), ">1.0 不匹配 1.0.5");
    assert!(req(">1.0").matches(&version("1.1.0")));
    // >1 等价 >=2.0.0。
    assert!(!req(">1").matches(&version("1.9.9")));
    assert!(req(">1").matches(&version("2.0.0")));
    // <1 等价 <1.0.0；<1.0 等价 <1.0.0（都不匹配 1.0.0）。
    assert!(req("<1.0").matches(&version("0.9.9")));
    assert!(!req("<1.0").matches(&version("1.0.0")));
    // =1.0 允许 patch 提升；=1.0.0 精确锁定。
    assert!(req("=1.0").matches(&version("1.0.9")));
    assert!(!req("=1.0.0").matches(&version("1.0.9")));

    // 缺省位必须逐层合法：patch 不允许脱离 minor 存在。
    assert!(Comparator::parse("1.0.0").is_ok());
    assert!(Comparator::parse("1.0").is_ok());
    assert!(Comparator::parse("1").is_ok());
    assert_to_string(
        comparator_err("1.0.0.0"),
        "unexpected character '.' after patch version number",
    );
}

/// req 级与 comparator 级 prerelease 门槛是“存在一个兼容 comparator”，
/// 单 comparator 直接评估会丢掉这条 OR 语义，这里是独立反例。
#[test]
fn prerelease_gate_req_vs_single_comparator() {
    let beta = version("1.0.0-beta");

    // 单个 `<1.0.0` comparator：beta 因 prerelease 门槛被拒绝。
    let less = comparator("<1.0.0");
    assert!(
        !less.matches(&beta),
        "单 comparator <1.0.0 不匹配 1.0.0-beta"
    );

    // 同一比较器放进带显式 prerelease 的多 comparator req 后，
    // 因为存在 `>1.0.0-alpha` 这个 m.m.p 相同且带 prerelease 的 comparator，
    // req 整体接受 beta。
    let combined = req(">1.0.0-alpha, <1.0.0");
    assert!(
        combined.matches(&beta),
        "req 级 OR 门槛应让 1.0.0-beta 通过"
    );

    // 但跨 m.m.p 的 prerelease 仍不会被“顺带”放行。
    assert!(!combined.matches(&version("1.0.1-beta")));
    assert!(!combined.matches(&version("2.0.0-alpha")));
}

/// Prerelease / BuildMetadata 的独立 parse + Display + 往返。
#[test]
fn standalone_identifier_roundtrip_and_empty() {
    for text in ["", "alpha", "alpha.1", "0a", "rc-1", "x-y.z"] {
        let pre = prerelease(text);
        assert_eq!(pre.as_str(), text);
        assert_to_string(&pre, text);
        assert_eq!(prerelease(&pre.to_string()), pre);
        assert_eq!(pre.is_empty(), text.is_empty());
    }

    // build 允许前导零、纯数字与空串；空串是合法的 EMPTY。
    for text in ["", "007", "00.01", "build.42", "20210327"] {
        let build = build_metadata(text);
        assert_eq!(build.as_str(), text);
        assert_to_string(&build, text);
        assert_eq!(build_metadata(&build.to_string()), build);
        assert_eq!(build.is_empty(), text.is_empty());
    }

    // 非法 ASCII 字符（下划线、空段、NUL）。
    assert_to_string(
        prerelease_err("a_b"),
        "unexpected character in pre-release identifier",
    );
    assert_to_string(
        build_metadata_err("a..b"),
        "empty identifier segment in build metadata",
    );
    assert_to_string(
        build_metadata_err("a\0b"),
        "unexpected character in build metadata",
    );
}

/// Hash/Eq 在 req 上也必须一致：规范化后相同结构同 hash，不同结构不同 hash。
#[test]
fn req_hash_follows_structure_not_spelling() {
    // 拼写不同但结构相同（Display 后相同）=> Eq 且 Hash 相同。
    assert_eq!(req("1.x"), req("1.X"));
    assert_eq!(hash64(&req("1.x")), hash64(&req("1.X")));
    assert_eq!(req(">=1.2.3+meta"), req(">=1.2.3"));
    assert_eq!(hash64(&req(">=1.2.3+meta")), hash64(&req(">=1.2.3")));
    // 结构不同 => 不等（即使 PROBES 向量相同，见 finite_witness 测试）。
    assert_ne!(req("^1.0.0"), req("1.0.*"));
}
