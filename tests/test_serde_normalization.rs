//! serde feature 开关下的规范化回归。
//!
//! 这些断言只在 `--features serde`（即完整验收的 `--all-features`）下编译，
//! 默认无 serde 时整个文件不参与编译，说明 serde 支持是纯增量特性：
//! `cargo test --locked` 不会编译或依赖任何 serde 相关代码。
//!
//! 同时覆盖两类 serializer：
//! - `serde_json`：is_human_readable() == true 的文本格式；
//! - 本文件手写的 `Bin`：is_human_readable() == false 的长度前缀二进制格式，
//!   不引入额外 crate，用来确认 Display 规范化在两种格式下一致。

#![cfg(feature = "serde")]

mod util;

use crate::util::*;
use semver::{Comparator, Prerelease, Version, VersionReq};
use serde::de::{Deserializer, Error as DeError, Visitor};
use serde::ser::{Error as SerError, Serialize, Serializer};
use std::fmt;

#[derive(Debug)]
struct BinError(String);

impl fmt::Display for BinError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for BinError {}

impl SerError for BinError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        BinError(message.to_string())
    }
}

impl DeError for BinError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        BinError(message.to_string())
    }
}

#[test]
fn version_serde_human_readable_roundtrips_through_display() {
    for text in ["1.2.3", "1.2.3-alpha.1", "1.2.3+007", "1.2.3-rc.1+sha.42"] {
        let parsed = version(text);

        // 序列化走 Display：线上格式就是规范化文本。
        let json = serde_json::to_string(&parsed).unwrap();
        assert_eq!(
            json,
            format!("\"{}\"", parsed),
            "JSON 必须直接使用 Display 文本"
        );
        assert_eq!(
            json,
            format!("\"{text}\""),
            "Version 的 Display 对合法输入往返保持"
        );

        let reparsed: Version = serde_json::from_str(&json).unwrap();
        assert_eq!(reparsed, parsed, "JSON 反序列化必须还原结构: `{text}`");

        // 非人类可读二进制格式得到同样的字符串内容，再 parse 回同结构。
        let bytes = to_bin(&parsed).unwrap();
        assert_eq!(from_bin::<Version>(&bytes).unwrap(), parsed);
    }
}

#[test]
fn version_req_serde_normalizes_spelling() {
    let parsed = req("  >=  1.0.0  , < 2.0.0 ");
    let json = serde_json::to_string(&parsed).unwrap();
    assert_eq!(json, "\">=1.0.0, <2.0.0\"");
    let reparsed: VersionReq = serde_json::from_str(&json).unwrap();
    assert_eq!(reparsed, parsed);
    let bytes = to_bin(&parsed).unwrap();
    assert_eq!(from_bin::<VersionReq>(&bytes).unwrap(), parsed);

    // 单个 Comparator 也通过 Display 序列化（build 丢弃、缺省 op 显形为 caret）。
    let comparator = comparator("1.2.3+ignored");
    let json = serde_json::to_string(&comparator).unwrap();
    assert_eq!(json, "\"^1.2.3\"");
    let reparsed: Comparator = serde_json::from_str(&json).unwrap();
    assert_eq!(reparsed, comparator);
    let bytes = to_bin(&comparator).unwrap();
    assert_eq!(from_bin::<Comparator>(&bytes).unwrap(), comparator);
}

#[test]
fn serde_rejects_invalid_without_widening_parse_set() {
    for bad in ["01.0.0", "1.2", "1.0.0-01", "1.0.0+", ""] {
        let json = format!("\"{bad}\"");
        let result: Result<Version, _> = serde_json::from_str(&json);
        assert!(result.is_err(), "JSON 反序列化应拒绝 `{bad}`");
        assert!(
            Version::parse(bad).is_err(),
            "serde 不应放宽解析接受集: `{bad}`"
        );
        assert!(
            from_bin::<Version>(&to_bin_str(bad)).is_err(),
            "二进制反序列化应拒绝 `{bad}`"
        );
    }

    // 错误类型（数字而非字符串）也报错，而不是去解析数字文本。
    assert!(serde_json::from_str::<Version>("123").is_err());

    // VersionReq 同样把非法拼写透传给 parser。
    assert!(serde_json::from_str::<VersionReq>("\"@1.0.0\"").is_err());
}

#[test]
fn prerelease_and_buildmetadata_have_no_standalone_serde() {
    // serde 只覆盖 Version / VersionReq / Comparator；Prerelease、BuildMetadata
    // 通过 Version 整体序列化，没有独立线上表示。这里用编译期断言固定公开面。
    fn assert_serialize<T: ?Sized + Serialize>() {}
    assert_serialize::<Version>();
    assert_serialize::<VersionReq>();
    assert_serialize::<Comparator>();

    // 它们仍可借助 as_str / Display 手工组合字符串。
    assert_eq!(prerelease("alpha.1").as_str(), "alpha.1");
    assert_eq!(build_metadata("007").as_str(), "007");
    let _ = Prerelease::EMPTY;
}

// ---------------------------------------------------------------------------
// 最小长度前缀二进制格式：u32 LE 长度 + UTF-8 字节。
// Version/VersionReq/Comparator 在其上都表现为 Display 字符串。
// ---------------------------------------------------------------------------

fn to_bin<T: Serialize>(value: &T) -> Result<Vec<u8>, BinError> {
    let mut serializer = BinSerializer { output: Vec::new() };
    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

fn to_bin_str(text: &str) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(&(text.len() as u32).to_le_bytes());
    output.extend_from_slice(text.as_bytes());
    output
}

fn from_bin<'de, T: serde::Deserialize<'de>>(bytes: &'de [u8]) -> Result<T, BinError> {
    let mut deserializer = BinDeserializer { bytes };
    T::deserialize(&mut deserializer)
}

struct BinSerializer {
    output: Vec<u8>,
}

macro_rules! reject_scalar {
    ($($method:ident($ty:ty)),* $(,)?) => {
        $(
            fn $method(self, _value: $ty) -> Result<(), BinError> {
                Err(BinError(format!("二进制版本格式只接受字符串，拒绝 {}", stringify!($method))))
            }
        )*
    };
}

impl Serializer for &mut BinSerializer {
    type Ok = ();
    type Error = BinError;
    type SerializeSeq = serde::ser::Impossible<(), BinError>;
    type SerializeTuple = serde::ser::Impossible<(), BinError>;
    type SerializeTupleStruct = serde::ser::Impossible<(), BinError>;
    type SerializeTupleVariant = serde::ser::Impossible<(), BinError>;
    type SerializeMap = serde::ser::Impossible<(), BinError>;
    type SerializeStruct = serde::ser::Impossible<(), BinError>;
    type SerializeStructVariant = serde::ser::Impossible<(), BinError>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_str(self, value: &str) -> Result<(), BinError> {
        let len =
            u32::try_from(value.len()).map_err(|_| BinError("版本串长度超出 u32".to_owned()))?;
        self.output.extend_from_slice(&len.to_le_bytes());
        self.output.extend_from_slice(value.as_bytes());
        Ok(())
    }

    reject_scalar! {
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char),
        serialize_bytes(&[u8]),
    }

    fn serialize_none(self) -> Result<(), BinError> {
        Err(BinError("二进制版本格式不支持 none".to_owned()))
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), BinError> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), BinError> {
        Err(BinError("二进制版本格式不支持 unit".to_owned()))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<(), BinError> {
        Err(BinError(format!("二进制版本格式不支持 unit_struct {name}")))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<(), BinError> {
        Err(BinError(format!(
            "二进制版本格式不支持 unit_variant {variant}"
        )))
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), BinError> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<(), BinError> {
        Err(BinError("二进制版本格式不支持 newtype_variant".to_owned()))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, BinError> {
        Err(BinError("二进制版本格式不支持 seq".to_owned()))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, BinError> {
        Err(BinError("二进制版本格式不支持 tuple".to_owned()))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, BinError> {
        Err(BinError("二进制版本格式不支持 tuple_struct".to_owned()))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, BinError> {
        Err(BinError("二进制版本格式不支持 tuple_variant".to_owned()))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, BinError> {
        Err(BinError("二进制版本格式不支持 map".to_owned()))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, BinError> {
        Err(BinError("二进制版本格式不支持 struct".to_owned()))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, BinError> {
        Err(BinError("二进制版本格式不支持 struct_variant".to_owned()))
    }
}

struct BinDeserializer<'de> {
    bytes: &'de [u8],
}

impl<'de> BinDeserializer<'de> {
    fn take_string(&mut self) -> Result<&'de str, BinError> {
        if self.bytes.len() < 4 {
            return Err(BinError("二进制版本串缺少长度前缀".to_owned()));
        }
        let len = u32::from_le_bytes([self.bytes[0], self.bytes[1], self.bytes[2], self.bytes[3]])
            as usize;
        let rest = &self.bytes[4..];
        if rest.len() < len {
            return Err(BinError("二进制版本串长度不足".to_owned()));
        }
        let (string, rest) = rest.split_at(len);
        self.bytes = rest;
        core::str::from_utf8(string).map_err(|error| BinError(error.to_string()))
    }
}

impl<'de> Deserializer<'de> for &mut BinDeserializer<'de> {
    type Error = BinError;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_borrowed_str(self.take_string()?)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_borrowed_str(self.take_string()?)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_str(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}
