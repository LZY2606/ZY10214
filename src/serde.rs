//! `Version`, `VersionReq`, and `Comparator` serialize as the string produced
//! by their `Display` impl and deserialize through the same parser used by
//! `FromStr`. Therefore the accepted input set is identical whether a value is
//! parsed directly or deserialized: a serialized requirement uses the
//! canonicalized display spelling (for example `1.0.0` serializes as `^1.0.0`,
//! and a comparator's build metadata never appears), and invalid strings
//! produce the parse error unchanged. `Prerelease` and `BuildMetadata` have no
//! standalone serde representation.

use crate::{Comparator, Version, VersionReq};
use core::fmt;
use serde::de::{Deserialize, Deserializer, Error, Visitor};
use serde::ser::{Serialize, Serializer};

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl Serialize for VersionReq {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl Serialize for Comparator {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VersionVisitor;

        impl<'de> Visitor<'de> for VersionVisitor {
            type Value = Version;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("semver version")
            }

            fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                string.parse().map_err(Error::custom)
            }
        }

        deserializer.deserialize_str(VersionVisitor)
    }
}

impl<'de> Deserialize<'de> for VersionReq {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VersionReqVisitor;

        impl<'de> Visitor<'de> for VersionReqVisitor {
            type Value = VersionReq;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("semver version")
            }

            fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                string.parse().map_err(Error::custom)
            }
        }

        deserializer.deserialize_str(VersionReqVisitor)
    }
}

impl<'de> Deserialize<'de> for Comparator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ComparatorVisitor;

        impl<'de> Visitor<'de> for ComparatorVisitor {
            type Value = Comparator;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("semver comparator")
            }

            fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                string.parse().map_err(Error::custom)
            }
        }

        deserializer.deserialize_str(ComparatorVisitor)
    }
}
