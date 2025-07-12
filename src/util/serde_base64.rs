// Cobble Build Automation
// Copyright (C) 2024 Jeremiah Darais
//
// This program is licensed under the GPLv3.0 license (https://github.com/jdarais/cobble/blob/main/COPYING)

use std::fmt;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{de, Deserializer, Serializer};

pub fn serialize<S>(val: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let val_b64 = STANDARD.encode(val);
    serializer.serialize_str(val_b64.as_str())
}

struct Base64Visitor;
impl<'de> de::Visitor<'de> for Base64Visitor {
    type Value = Vec<u8>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "A base64-encoded string value")
    }

    fn visit_str<E>(self, val: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        STANDARD.decode(val).map_err(|e| E::custom(format!("{e}")))
    }
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_str(Base64Visitor)
}
