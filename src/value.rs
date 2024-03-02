use crate::{decode_var_int, encode_var_int, Error, VarInt};
use std::io::{Read, Seek};

/// Represents any object value.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueVariant {
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Bool(bool),
    Float(f32),
    Double(f64),
    Data(Vec<u8>),
    Nil,
    ObjectRef(u32),
}

/// Represents a single value of a NIB Archive.
///
/// A value contains an index to a key with its name and a value itself.
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    key_index: VarInt,
    value: ValueVariant,
}

impl Value {
    pub(crate) fn try_from_reader<T: Read + Seek>(mut reader: &mut T) -> Result<Self, Error> {
        let key_index = decode_var_int(&mut reader)?;
        let mut _value_type_byte = [0; 1];
        reader.read_exact(&mut _value_type_byte)?;
        let value_type_byte = _value_type_byte[0];
        let value = match value_type_byte {
            0 => {
                let mut buf = [0; 1];
                reader.read_exact(&mut buf)?;
                ValueVariant::Int8(i8::from_le_bytes(buf))
            }
            1 => {
                let mut buf = [0; 2];
                reader.read_exact(&mut buf)?;
                ValueVariant::Int16(i16::from_le_bytes(buf))
            }
            2 => {
                let mut buf = [0; 4];
                reader.read_exact(&mut buf)?;
                ValueVariant::Int32(i32::from_le_bytes(buf))
            }
            3 => {
                let mut buf = [0; 8];
                reader.read_exact(&mut buf)?;
                ValueVariant::Int64(i64::from_le_bytes(buf))
            }
            4 => ValueVariant::Bool(false),
            5 => ValueVariant::Bool(true),
            6 => {
                let mut buf = [0; 4];
                reader.read_exact(&mut buf)?;
                ValueVariant::Float(f32::from_le_bytes(buf))
            }
            7 => {
                let mut buf = [0; 8];
                reader.read_exact(&mut buf)?;
                ValueVariant::Double(f64::from_le_bytes(buf))
            }
            8 => {
                let length = decode_var_int(&mut reader)?;
                let mut buf = vec![0; length as usize];
                reader.read_exact(&mut buf)?;
                ValueVariant::Data(buf)
            }
            9 => ValueVariant::Nil,
            10 => {
                let mut buf = [0; 4];
                reader.read_exact(&mut buf)?;
                ValueVariant::ObjectRef(u32::from_le_bytes(buf))
            }
            _ => {
                return Err(Error::FormatError(format!(
                    "unknown value type {value_type_byte:#04x}"
                )))
            }
        };
        Ok(Self { key_index, value })
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = encode_var_int(self.key_index);

        match &self.value {
            ValueVariant::Int8(v) => {
                bytes.push(0);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Int16(v) => {
                bytes.push(1);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Int32(v) => {
                bytes.push(2);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Int64(v) => {
                bytes.push(3);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Bool(v) => {
                if !v {
                    bytes.push(4);
                } else {
                    bytes.push(5);
                }
            }
            ValueVariant::Float(v) => {
                bytes.push(6);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Double(v) => {
                bytes.push(7);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Data(v) => {
                bytes.push(8);
                bytes.append(&mut encode_var_int(v.len() as i32));
                bytes.extend_from_slice(v);
            }
            ValueVariant::Nil => {
                bytes.push(9);
            }
            ValueVariant::ObjectRef(v) => {
                bytes.push(10);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }

        bytes
    }

    /// Creates a new NIB Archive value.
    pub fn new(key_index: VarInt, value: ValueVariant) -> Self {
        Self { key_index, value }
    }

    /// Returns an index to a key with value's name.
    pub fn key_index(&self) -> VarInt {
        self.key_index
    }

    /// Sets value's key index.
    pub fn set_key_index(&mut self, index: VarInt) {
        self.key_index = index
    }

    /// Returns a reference to a key associated with the current value.
    ///
    /// Pass the return value of [NIBArchive::keys()] method for a proper result.
    pub fn key<'a>(&self, keys: &'a [String]) -> &'a String {
        &keys[self.key_index() as usize]
    }

    /// Return the underlying value.
    pub fn value(&self) -> &ValueVariant {
        &self.value
    }

    /// Sets value.
    pub fn set_value(&mut self, value: ValueVariant) {
        self.value = value
    }

    /// Consumes itself and returns a unit of `key_index` and `value`.
    pub fn into_inner(self) -> (VarInt, ValueVariant) {
        (self.key_index, self.value)
    }
}
