use crate::{decode_var_int, encode_var_int, Error, VarInt};
use std::io::{Read, Seek};

const TYPE_INT8: u8 = 0;
const TYPE_INT16: u8 = 1;
const TYPE_INT32: u8 = 2;
const TYPE_INT64: u8 = 3;
const TYPE_BOOL_FALSE: u8 = 4;
const TYPE_BOOL_TRUE: u8 = 5;
const TYPE_FLOAT: u8 = 6;
const TYPE_DOUBLE: u8 = 7;
const TYPE_DATA: u8 = 8;
const TYPE_NIL: u8 = 9;
const TYPE_OBJECT_REF: u8 = 10;

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
        let mut value_type_byte = [0; 1];
        reader.read_exact(&mut value_type_byte)?;
        let value_type_byte = value_type_byte[0];
        let value = match value_type_byte {
            TYPE_INT8 => {
                let mut buf = [0; 1];
                reader.read_exact(&mut buf)?;
                ValueVariant::Int8(i8::from_le_bytes(buf))
            }
            TYPE_INT16 => {
                let mut buf = [0; 2];
                reader.read_exact(&mut buf)?;
                ValueVariant::Int16(i16::from_le_bytes(buf))
            }
            TYPE_INT32 => {
                let mut buf = [0; 4];
                reader.read_exact(&mut buf)?;
                ValueVariant::Int32(i32::from_le_bytes(buf))
            }
            TYPE_INT64 => {
                let mut buf = [0; 8];
                reader.read_exact(&mut buf)?;
                ValueVariant::Int64(i64::from_le_bytes(buf))
            }
            TYPE_BOOL_FALSE => ValueVariant::Bool(false),
            TYPE_BOOL_TRUE => ValueVariant::Bool(true),
            TYPE_FLOAT => {
                let mut buf = [0; 4];
                reader.read_exact(&mut buf)?;
                ValueVariant::Float(f32::from_le_bytes(buf))
            }
            TYPE_DOUBLE => {
                let mut buf = [0; 8];
                reader.read_exact(&mut buf)?;
                ValueVariant::Double(f64::from_le_bytes(buf))
            }
            TYPE_DATA => {
                let length = decode_var_int(&mut reader)?;
                let mut buf = vec![0; length as usize];
                reader.read_exact(&mut buf)?;
                ValueVariant::Data(buf)
            }
            TYPE_NIL => ValueVariant::Nil,
            TYPE_OBJECT_REF => {
                let mut buf = [0; 4];
                reader.read_exact(&mut buf)?;
                ValueVariant::ObjectRef(u32::from_le_bytes(buf))
            }
            _ => {
                return Err(Error::FormatError(format!(
                    "Unknown value type {value_type_byte:#04x}"
                )))
            }
        };
        Ok(Self { key_index, value })
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = encode_var_int(self.key_index);

        match &self.value {
            ValueVariant::Int8(v) => {
                bytes.push(TYPE_INT8);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Int16(v) => {
                bytes.push(TYPE_INT16);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Int32(v) => {
                bytes.push(TYPE_INT32);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Int64(v) => {
                bytes.push(TYPE_INT64);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Bool(v) => {
                if !v {
                    bytes.push(TYPE_BOOL_FALSE);
                } else {
                    bytes.push(TYPE_BOOL_TRUE);
                }
            }
            ValueVariant::Float(v) => {
                bytes.push(TYPE_FLOAT);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Double(v) => {
                bytes.push(TYPE_DOUBLE);
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            ValueVariant::Data(v) => {
                bytes.push(TYPE_DATA);
                bytes.append(&mut encode_var_int(v.len() as i32));
                bytes.extend_from_slice(v);
            }
            ValueVariant::Nil => {
                bytes.push(TYPE_NIL);
            }
            ValueVariant::ObjectRef(v) => {
                bytes.push(TYPE_OBJECT_REF);
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
    /// Pass the return value of [crate::NIBArchive::keys()] method for a proper result.
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
