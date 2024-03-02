use crate::Error;
use std::io::{Read, Seek};

/// Represents a header of a NIB Archive.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Header {
    pub format_version: u32,
    pub coder_version: u32,
    pub object_count: u32,
    pub offset_objects: u32,
    pub key_count: u32,
    pub offset_keys: u32,
    pub value_count: u32,
    pub offset_values: u32,
    pub class_name_count: u32,
    pub offset_class_names: u32,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            format_version: 1,
            coder_version: 9,
            object_count: Default::default(),
            offset_objects: Default::default(),
            key_count: Default::default(),
            offset_keys: Default::default(),
            value_count: Default::default(),
            offset_values: Default::default(),
            class_name_count: Default::default(),
            offset_class_names: Default::default(),
        }
    }
}

impl Header {
    pub(crate) fn try_from_reader<T: Read + Seek>(reader: &mut T) -> Result<Self, Error> {
        // Reads 40 bytes of a header
        let mut buf = [0; 4];
        let mut values = [0; 10];
        for item in &mut values {
            reader.read_exact(&mut buf)?;
            *item = u32::from_le_bytes(buf);
        }
        Ok(Self {
            format_version: values[0],
            coder_version: values[1],
            object_count: values[2],
            offset_objects: values[3],
            key_count: values[4],
            offset_keys: values[5],
            value_count: values[6],
            offset_values: values[7],
            class_name_count: values[8],
            offset_class_names: values[9],
        })
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(40);
        result.extend_from_slice(&self.format_version.to_le_bytes());
        result.extend_from_slice(&self.coder_version.to_le_bytes());
        result.extend_from_slice(&self.object_count.to_le_bytes());
        result.extend_from_slice(&self.offset_objects.to_le_bytes());
        result.extend_from_slice(&self.key_count.to_le_bytes());
        result.extend_from_slice(&self.offset_keys.to_le_bytes());
        result.extend_from_slice(&self.value_count.to_le_bytes());
        result.extend_from_slice(&self.offset_values.to_le_bytes());
        result.extend_from_slice(&self.class_name_count.to_le_bytes());
        result.extend_from_slice(&self.offset_class_names.to_le_bytes());
        result
    }
}
