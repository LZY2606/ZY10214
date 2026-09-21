//! Prerelease 与 BuildMetadata 的身份字段、语法接受集与排序反例。
//!
//! 两者底层共用 Identifier，但语义不同：
//! - 语法上只有 prerelease 禁止纯数字段的前导零；build 允许。
//! - 排序上 prerelease 的"空"表示正式版、大于任何非空 prerelease；
//!   build 没有这层含义，空串与空串相等且最小。
//! - build 的数字段把前导零作为*第三*排序键（数值相同再比原串长度），
//!   prerelease 的数字段不可能带前导零，因此只有前两级。

use semver::{BuildMetadata, Prerelease, Version};
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn hash64(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn prerelease_syntax_acceptance_table() {
    let ok = [
        "",
        "-",
        "0",
        "a",
        "alpha.1",
        "0a",
        "01a",
        "1-1",
        "a.0a.9",
        "99999999999999999999999",
    ];
    for input in ok {
        let parsed = Prerelease::new(input)
            .unwrap_or_else(|err| panic!("Prerelease::new({input:?}) unexpectedly failed: {err}"));
        assert_eq!(
            parsed.as_str(),
            input,
            "stored text must preserve spelling for {input:?}"
        );
        assert_eq!(parsed.to_string(), input);
        // 再解析结构相等、Hash 一致。
        assert_eq!(Prerelease::new(input).unwrap(), parsed);
        assert_eq!(hash64(&Prerelease::new(input).unwrap()), hash64(&parsed));
    }

    let rejected = [
        "00",      // 纯数字段前导零
        "1.01",    // 第二段前导零
        "a..1",    // 空段
        "a.",      // 结尾空段
        ".a",      // 开头空段
        "a_b",     // 下划线非法
        "a b",     // 空格非法
        "a\u{e9}", // 非 ASCII 非法
    ];
    for input in rejected {
        assert!(
            Prerelease::new(input).is_err(),
            "Prerelease::new({input:?}) unexpectedly succeeded",
        );
    }
}

#[test]
fn build_syntax_acceptance_table_allows_leading_zero() {
    let ok = ["", "-", "0", "00", "001", "1.01", "a.001", "1-1-1"];
    for input in ok {
        let parsed = BuildMetadata::new(input).unwrap_or_else(|err| {
            panic!("BuildMetadata::new({input:?}) unexpectedly failed: {err}")
        });
        assert_eq!(parsed.as_str(), input);
        assert_eq!(BuildMetadata::new(input).unwrap(), parsed);
        assert_eq!(hash64(&BuildMetadata::new(input).unwrap()), hash64(&parsed));
    }

    // 相同的"前导零/空段/非法字符"在 build 侧也并非全部放宽：空段与非法
    // 字符仍然拒绝，只有前导零合法。
    for rejected in ["a..1", "a.", ".a", "a_b", "a b"] {
        assert!(
            BuildMetadata::new(rejected).is_err(),
            "BuildMetadata::new({rejected:?}) unexpectedly succeeded",
        );
    }
}

/// 独立反例：同一串文本 "00" 在 build 合法、在 prerelease 非法。
#[test]
fn leading_zero_is_legal_only_in_build() {
    assert!(BuildMetadata::new("00").is_ok());
    assert!(Prerelease::new("00").is_err());
    // 带字母后前导零在两处都合法（不再是"纯数字 identifier"）。
    assert!(Prerelease::new("00a").is_ok());
    assert!(BuildMetadata::new("00a").is_ok());
}

/// 独立反例：超长数字串不按 u64 解析，按"长度再字典序"比较，永不溢出。
#[test]
fn oversized_numeric_identifiers_compare_by_length() {
    let huge = "99999999999999999999999"; // 23 位，远超 u64::MAX
    let small = "1";
    let pre_huge = Prerelease::new(huge).unwrap();
    let pre_small = Prerelease::new(small).unwrap();
    assert_eq!(pre_huge.cmp(&pre_small), Ordering::Greater);

    let b_huge = BuildMetadata::new(huge).unwrap();
    let b_small = BuildMetadata::new(small).unwrap();
    assert_eq!(b_huge.cmp(&b_small), Ordering::Greater);
}

/// Prerelease 排序链：数字/字母方向、空 prerelease 的特殊地位。
#[test]
fn prerelease_ordering_chain() {
    let chain = [
        "0", "1", "9", "10", "100", // 数字段按数值
        "1a0", "a", // 数字段 < 非数字段
        "alpha", "alpha.0", "alpha.1", "alpha.2", "alpha.11", "alpha11",
        "alpha2", // 无点字母段按 ASCII，alpha11 < alpha2
        "beta",
    ];
    let parsed: Vec<Prerelease> = chain.iter().map(|s| Prerelease::new(s).unwrap()).collect();
    for window in parsed.windows(2) {
        assert!(
            window[0] < window[1],
            "expected {} < {}",
            window[0],
            window[1]
        );
    }

    // 空 prerelease（代表正式版）大于一切非空 prerelease。
    let empty = Prerelease::EMPTY;
    assert!(empty > Prerelease::new("zzz").unwrap());
    assert_eq!(empty.cmp(&Prerelease::new("").unwrap()), Ordering::Equal);
}

/// Build 排序链：数值相同再以前导零（原串长度）作为第三排序键。
#[test]
fn build_ordering_chain_with_leading_zero() {
    // 0 < 00 < 1 < 01 < 001 < 2 < 02 < 002 < 10
    let chain = [
        "0", "00", "1", "01", "001", "2", "02", "002", "10", "1a0", "a",
    ];
    let parsed: Vec<BuildMetadata> = chain
        .iter()
        .map(|s| BuildMetadata::new(s).unwrap())
        .collect();
    for window in parsed.windows(2) {
        assert!(
            window[0] < window[1],
            "expected {} < {}",
            window[0],
            window[1]
        );
    }
}

/// 两种 identifier 都参与 Version 的 Display 与结构相等，但只有 pre 进入
/// 版本优先级；build 的优先级影响通过 cmp_precedence 已在主测试锁定，这里
/// 补充验证 build 自身 Ord 不影响 prerelease 的优先级方向。
#[test]
fn version_precedence_uses_pre_but_not_build() {
    let a = Version::parse("1.0.0-alpha+aaa").unwrap();
    let b = Version::parse("1.0.0-alpha+bbb").unwrap();
    assert_eq!(a.cmp_precedence(&b), Ordering::Equal);
    let c = Version::parse("1.0.0+zzz").unwrap();
    assert!(a.cmp_precedence(&c).is_lt());
}
