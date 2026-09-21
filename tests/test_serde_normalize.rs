//! serde feature 开关下的规范化边界反例。
//!
//! 这些测试整体只在 `--features serde` 下编译（文件末尾整体 `cfg`），因此
//! 默认构建不依赖 serde。它们锁定两件事：
//!
//! 1. 人类可读格式（JSON）与二进制格式（本文件自带的微型、自包含 wire
//!    format，`is_human_readable == false`）都把 Version / VersionReq /
//!    Comparator 表示成规范化后的 Display 字符串，而不是原拼写。
//! 2. 反序列化走同一条 parse 路径，接受集与 `Version::parse` 完全一致；
//!    被规范化的输入在 JSON 往返后会"变样"，这是序列化层面的字符串往返陷阱。

#![cfg(feature = "serde")]

use semver::{Comparator, Version, VersionReq};
use serde::de::{Deserialize, Deserializer, Visitor};
use serde::ser::{Error as SerError, Serialize, Serializer};

// ==========================================================================
// 人类可读格式：JSON
// ==========================================================================

#[test]
fn json_version_roundtrip_uses_normalized_display() {
    let version = Version::parse("1.2.3-rc.1+build.7").unwrap();
    let json = serde_json::to_string(&version).unwrap();
    assert_eq!(json, r#""1.2.3-rc.1+build.7""#);
    let reparsed: Version = serde_json::from_str(&json).unwrap();
    assert_eq!(reparsed, version);
}

#[test]
fn json_req_normalizes_spelling_and_spaces() {
    // 非规范化拼写：operator 两侧空格、逗号两侧空格。
    let req = VersionReq::parse(" >= 1.0.0 , < 2.0.0 ").unwrap();
    let json = serde_json::to_string(&req).unwrap();
    assert_eq!(
        json, r#"">=1.0.0, <2.0.0""#,
        "serialization must emit Display normalization",
    );

    // 反序列化规范化文本得到同一结构。
    let reparsed: VersionReq = serde_json::from_str(&json).unwrap();
    assert_eq!(reparsed, req);

    // 但原始拼写反序列化再序列化后也会被规范化：字符串层面不往返。
    let from_original_spelling: VersionReq =
        serde_json::from_str(r#"">= 1.0.0 , < 2.0.0""#).unwrap();
    assert_eq!(from_original_spelling, req);
    assert_ne!(
        serde_json::to_string(&from_original_spelling).unwrap(),
        r#"">= 1.0.0 , < 2.0.0""#,
    );

    // 缺省 caret + x 通配符同样规范化。
    let wildcard: VersionReq = serde_json::from_str(r#""1.x""#).unwrap();
    assert_eq!(serde_json::to_string(&wildcard).unwrap(), r#""1.*""#);
}

#[test]
fn json_rejects_the_same_inputs_parse_rejects() {
    // serde 反序列化不放宽任何语法：接受集必须与 parse 完全一致。
    for invalid in ["1.2", "1.02.0", "1.0.0-", "18446744073709551616.0.0"] {
        let json = format!("\"{invalid}\"");
        serde_json::from_str::<Version>(&json)
            .err()
            .unwrap_or_else(|| panic!("serde unexpectedly accepted {invalid}"));
    }
    for invalid in ["@1.0.0", "*.*", ">=1.0 <2.0"] {
        let json = format!("\"{invalid}\"");
        serde_json::from_str::<VersionReq>(&json)
            .err()
            .unwrap_or_else(|| panic!("serde unexpectedly accepted req {invalid}"));
    }
}

#[test]
fn json_comparator_roundtrip() {
    // 显式 op 时 wildcard 被 Display 吞掉：=1.2.* -> "=1.2"。
    let cmp = Comparator::parse("=1.2.*").unwrap();
    let json = serde_json::to_string(&cmp).unwrap();
    assert_eq!(json, r#""=1.2""#);
    let reparsed: Comparator = serde_json::from_str(&json).unwrap();
    assert_eq!(reparsed, cmp);
}

/// 嵌套场景：JSON 数组里的 Version / VersionReq 同样以规范化字符串编码。
/// 用元组而非 derive，避免把 serde derive 拉进 dev-dependency。
#[test]
fn json_nested_values_are_normalized() {
    let tuple = (
        "demo".to_owned(),
        Version::new(1, 4, 2),
        VersionReq::parse("1.x").unwrap(),
    );
    let json = serde_json::to_string(&tuple).unwrap();
    assert_eq!(json, r#"["demo","1.4.2","1.*"]"#);

    let back: (String, Version, VersionReq) = serde_json::from_str(&json).unwrap();
    assert_eq!(back.1, tuple.1);
    assert_eq!(back.2, tuple.2);
}

// ==========================================================================
// 二进制路径：极简字符串收集 serializer / 字节喂入 deserializer
//
// 不引入 bincode 等额外格式。StringCollector 只接受 collect_str 最终调用
// 的 serialize_str，并通过 is_human_readable 模拟二进制格式；反序列化侧
// ByteDeserializer 把一段 UTF-8 字节当作字符串喂给 semver 的 visitor。
// 错误类型复用 serde_json::Error（它实现了 serde 的 ser/de Error）。
// ==========================================================================

struct StringCollector {
    human: bool,
}

type CollectorError = serde_json::Error;
type Impossible = serde::ser::Impossible<String, CollectorError>;

impl Serializer for StringCollector {
    type Ok = String;
    type Error = CollectorError;
    type SerializeSeq = Impossible;
    type SerializeTuple = Impossible;
    type SerializeTupleStruct = Impossible;
    type SerializeTupleVariant = Impossible;
    type SerializeMap = Impossible;
    type SerializeStruct = Impossible;
    type SerializeStructVariant = Impossible;

    fn is_human_readable(&self) -> bool {
        self.human
    }

    fn serialize_str(self, value: &str) -> Result<String, Self::Error> {
        Ok(value.to_owned())
    }

    fn serialize_bool(self, _: bool) -> Result<String, Self::Error> {
        Err(SerError::custom("StringCollector only supports strings"))
    }
    fn serialize_i8(self, _: i8) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_i16(self, _: i16) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_i32(self, _: i32) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_i64(self, _: i64) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_i128(self, _: i128) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_u8(self, _: u8) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_u16(self, _: u16) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_u32(self, _: u32) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_u64(self, _: u64) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_u128(self, _: u128) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_f32(self, _: f32) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_f64(self, _: f64) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_char(self, _: char) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_none(self) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_unit(self) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<String, Self::Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<String, Self::Error> {
        self.serialize_bool(false)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(SerError::custom("StringCollector only supports strings"))
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(None)
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(None)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.serialize_seq(None)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.serialize_seq(None)
    }
    fn serialize_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_seq(None)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.serialize_seq(None)
    }
}

fn collect_binary<T: Serialize>(value: &T) -> String {
    // semver 的 Serialize 使用 collect_str，与 is_human_readable 无关；
    // 以 human=false 调用以锁定"二进制格式拿到的也是 Display 文本"。
    value
        .serialize(StringCollector { human: false })
        .expect("collect_str only calls serialize_str")
}

#[test]
fn binary_serialization_also_emits_normalized_display() {
    assert_eq!(
        collect_binary(&Version::parse("1.2.3+007").unwrap()),
        "1.2.3+007"
    );
    assert_eq!(
        collect_binary(&VersionReq::parse(" >=1 , <2 ").unwrap()),
        ">=1, <2"
    );
    assert_eq!(
        collect_binary(&Comparator::parse("1.2.x").unwrap()),
        "1.2.*"
    );
}

/// 直接喂字节的 deserializer：只支持字符串，声明自己不是人类可读格式。
struct ByteDeserializer<'a> {
    bytes: &'a [u8],
}

impl<'de> Deserializer<'de> for ByteDeserializer<'de> {
    type Error = serde_json::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let string = std::str::from_utf8(self.bytes)
            .map_err(|_| SerError::custom("test wire format only carries utf-8"))?;
        visitor.visit_str(string)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

#[test]
fn binary_deserialization_uses_same_parser() {
    let version = Version::deserialize(ByteDeserializer {
        bytes: b"1.2.3-alpha",
    })
    .unwrap();
    assert_eq!(version, Version::parse("1.2.3-alpha").unwrap());

    let req = VersionReq::deserialize(ByteDeserializer { bytes: b"^1.2.3" }).unwrap();
    assert_eq!(req, VersionReq::parse("^1.2.3").unwrap());

    // 非法输入在二进制路径同样失败，错误来自同一个 parse。
    let err = Version::deserialize(ByteDeserializer { bytes: b"1.2" }).unwrap_err();
    assert!(err.to_string().contains("minor version number"));
}
