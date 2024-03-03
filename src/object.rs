use crate::{decode_var_int, encode_var_int, ClassName, Error, Value, VarInt};
use std::io::{Read, Seek};

/// Represents a single object of a NIB Archive.
///
/// An object contains an index of a representing class name, the first index of
/// a value and the count of all values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Object {
    class_name_index: VarInt,
    values_index: VarInt,
    value_count: VarInt,
}

impl Object {
    pub(crate) fn try_from_reader<T: Read + Seek>(mut reader: &mut T) -> Result<Self, Error> {
        Ok(Self {
            class_name_index: decode_var_int(&mut reader)?,
            values_index: decode_var_int(&mut reader)?,
            value_count: decode_var_int(&mut reader)?,
        })
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = encode_var_int(self.class_name_index);
        bytes.append(&mut encode_var_int(self.values_index));
        bytes.append(&mut encode_var_int(self.value_count));
        bytes
    }

    /// Creates a new NIB Archive object.
    pub fn new(class_name_index: VarInt, values_index: VarInt, value_count: VarInt) -> Self {
        Self {
            class_name_index,
            values_index,
            value_count,
        }
    }

    /// Returns an index of a [ClassName] that describes the current object.
    pub fn class_name_index(&self) -> VarInt {
        self.class_name_index
    }

    /// Sets object's class name index.
    pub fn set_class_name_index(&mut self, index: VarInt) {
        self.class_name_index = index
    }

    /// Returns the first index of a [Value] that the object contains.
    pub fn values_index(&self) -> VarInt {
        self.values_index
    }

    /// Sets value's first index of an object.
    pub fn set_values_index(&mut self, index: VarInt) {
        self.values_index = index
    }

    /// Returns the count of all [Values](Value) that the object contains.
    pub fn value_count(&self) -> VarInt {
        self.value_count
    }

    /// Sets values' count of an object.
    pub fn set_value_count(&mut self, count: VarInt) {
        self.value_count = count
    }

    /// Returns a slice of [Values](Value) associated with the current object.
    ///
    /// Pass the return value of [crate::NIBArchive::values()] method for a proper result.
    pub fn values<'a>(&self, values: &'a [Value]) -> &'a [Value] {
        let start = self.values_index() as usize;
        let end = start + self.value_count() as usize;
        &values[start..end]
    }

    /// Returns a reference to a [ClassName] associated with the current object.
    ///
    /// Pass the return value of [crate::NIBArchive::class_names()] method for a proper result.
    pub fn class_name<'a>(&self, class_names: &'a [ClassName]) -> &'a ClassName {
        &class_names[self.class_name_index() as usize]
    }

    /// Consumes itself and returns a unit of `class_name_index`, `values_index` and `value_count`.
    pub fn into_inner(self) -> (VarInt, VarInt, VarInt) {
        (self.class_name_index, self.values_index, self.value_count)
    }
}
