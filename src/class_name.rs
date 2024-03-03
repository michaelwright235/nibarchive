use crate::{decode_var_int, encode_var_int, Error};
use std::io::{Read, Seek};

/// Represents a single class name of a NIB Archive.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassName {
    name: String,
    fallback_classes_indeces: Vec<i32>,
}

impl ClassName {
    pub(crate) fn try_from_reader<T: Read + Seek>(mut reader: &mut T) -> Result<Self, Error> {
        let length = decode_var_int(&mut reader)?;
        let fallback_classes_count = decode_var_int(&mut reader)?;
        let mut fallback_classes_indeces = Vec::with_capacity(fallback_classes_count as usize);
        for _ in 0..fallback_classes_count {
            let mut buf = [0; 4];
            reader.read_exact(&mut buf)?;
            fallback_classes_indeces.push(i32::from_le_bytes(buf));
        }
        let mut name_bytes = vec![0; length as usize];
        reader.read_exact(&mut name_bytes)?;
        name_bytes.pop(); // Name is \0 terminated, so we have to remove the trailing \0
        let name = String::from_utf8(name_bytes)?;
        Ok(Self {
            name,
            fallback_classes_indeces,
        })
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = encode_var_int(self.name.len() as i32 + 1);
        bytes.append(&mut encode_var_int(
            self.fallback_classes_indeces.len() as i32
        ));
        for cls in &self.fallback_classes_indeces {
            bytes.extend_from_slice(&cls.to_le_bytes());
        }
        bytes.extend_from_slice(self.name.as_bytes());
        bytes.push(0x00);
        bytes
    }

    /// Creates a new NIB Archive class name.
    pub fn new(name: String, fallback_classes_indeces: Vec<i32>) -> Self {
        Self {
            name,
            fallback_classes_indeces,
        }
    }

    /// Returns the name of a class.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sets class name.
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Returns an array of indeces for fallback classes.
    pub fn fallback_classes_indeces(&self) -> &[i32] {
        &self.fallback_classes_indeces
    }

    /// Sets fallback classes indeces of a class.
    pub fn set_fallback_classes_indeces(&mut self, indeces: Vec<i32>) {
        self.fallback_classes_indeces = indeces;
    }

    /// Returns a slice of [ClassNames](ClassName) representing fallback classes.
    ///
    /// Pass the return value of [crate::NIBArchive::class_names()] method for a proper result.
    pub fn fallback_classes<'a>(&self, class_names: &'a [ClassName]) -> Vec<&'a ClassName> {
        let mut fallback_classes = Vec::with_capacity(self.fallback_classes_indeces.len());
        for i in self.fallback_classes_indeces() {
            fallback_classes.push(&class_names[*i as usize])
        }
        fallback_classes
    }

    /// Consumes itself and returns a unit of `name` and `fallback_classes`
    pub fn into_inner(self) -> (String, Vec<i32>) {
        (self.name, self.fallback_classes_indeces)
    }
}
