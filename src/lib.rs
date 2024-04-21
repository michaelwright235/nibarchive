#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

mod class_name;
mod error;
mod header;
mod object;
mod value;
pub use crate::{class_name::*, error::*, object::*, value::*};
use header::*;

use std::{
    fs::File,
    io::{BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write},
};

const MAGIC_BYTES: &[u8; 10] = b"NIBArchive";
const DEFAULT_FORMAT_VERSION: u32 = 1;
const DEFAULT_CODER_VERSION: u32 = 9;
type VarInt = i32;

/// After reading the current block of data we check that the current stream
/// position is equal to the start position of a next block.
macro_rules! check_position {
    ($reader:ident, $offset:expr, $err:literal) => {
        if $reader.stream_position()? != $offset as u64 {
            return Err(Error::FormatError(format!(
                "Expected {} offset at {} - got {}",
                $err,
                $reader.stream_position()?,
                $offset
            )));
        }
    };
}

/// NIB Archive decoder/encoder.
///
/// Look at the module docs for more details.
#[derive(Debug, Clone, PartialEq)]
pub struct NIBArchive {
    objects: Vec<Object>,
    keys: Vec<String>,
    values: Vec<Value>,
    class_names: Vec<ClassName>,
    format_version: u32,
    coder_version: u32,
}

impl NIBArchive {
    /// Creates a new NIB Archive from `objects`, `keys`, `values` and `class_names`.
    ///
    /// Returns an error if one of the elements references an element that is out of bounds.
    pub fn new(
        objects: Vec<Object>,
        keys: Vec<String>,
        values: Vec<Value>,
        class_names: Vec<ClassName>,
    ) -> Result<Self, Error> {
        for obj in &objects {
            Self::check_object(obj, values.len() as u32, class_names.len() as u32)?;
        }
        for val in &values {
            Self::check_value(val, keys.len() as u32)?;
        }
        for cls in &class_names {
            Self::check_class_name(cls, class_names.len() as u32)?;
        }
        Ok(Self {
            objects,
            keys,
            values,
            class_names,
            format_version: DEFAULT_FORMAT_VERSION,
            coder_version: DEFAULT_CODER_VERSION,
        })
    }

    /// Creates a new NIB Archive from `objects`, `keys`, `values` and `class_names`
    /// without any checks.
    ///
    /// This method **does not** check the input values. For example, a situation when an object's
    /// key index is out of bounds of the `keys` parameter is unchecked.
    pub fn new_unchecked(
        objects: Vec<Object>,
        keys: Vec<String>,
        values: Vec<Value>,
        class_names: Vec<ClassName>,
    ) -> Self {
        Self {
            objects,
            keys,
            values,
            class_names,
            format_version: DEFAULT_FORMAT_VERSION,
            coder_version: DEFAULT_CODER_VERSION,
        }
    }

    /// Reads and decodes a NIB Archive from a given file.
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Self::from_reader(&mut reader)
    }

    /// Reads and decodes a NIB Archive from a given slice of byte.
    pub fn from_bytes<B: AsRef<[u8]>>(bytes: B) -> Result<Self, Error> {
        let mut cursor = Cursor::new(bytes);
        Self::from_reader(&mut cursor)
    }

    /// Reads and decodes a NIB Archive from a given reader.
    pub fn from_reader<T: Read + Seek>(mut reader: &mut T) -> Result<Self, Error> {
        reader.seek(SeekFrom::Start(0))?;

        // Check magic bytes
        let mut magic_bytes = [0; 10];
        reader.read_exact(&mut magic_bytes)?;
        if &magic_bytes != MAGIC_BYTES {
            return Err(Error::FormatError("Magic bytes don't match".into()));
        }

        // Parse header
        let header = Header::try_from_reader(&mut reader)?;
        check_position!(reader, header.offset_objects, "object");

        // Parse objects
        let mut objects = Vec::with_capacity(header.object_count as usize);
        for _ in 0..header.object_count {
            let obj = Object::try_from_reader(&mut reader)?;
            Self::check_object(&obj, header.value_count, header.class_name_count)?;
            objects.push(obj);
        }
        check_position!(reader, header.offset_keys, "keys");

        // Parse keys
        let mut keys = Vec::with_capacity(header.key_count as usize);
        for _ in 0..header.key_count {
            let length = decode_var_int(&mut reader)?;
            let mut name_bytes = vec![0; length as usize];
            reader.read_exact(&mut name_bytes)?;
            let name = String::from_utf8(name_bytes)?;
            keys.push(name);
        }
        check_position!(reader, header.offset_values, "values");

        // Parse values
        let mut values = Vec::with_capacity(header.value_count as usize);
        for _ in 0..header.value_count {
            let val = Value::try_from_reader(&mut reader)?;
            Self::check_value(&val, header.key_count)?;
            values.push(val);
        }
        check_position!(reader, header.offset_class_names, "class names'");

        // Parse class names
        let mut class_names = Vec::with_capacity(header.class_name_count as usize);
        for _ in 0..header.class_name_count {
            let cls = ClassName::try_from_reader(&mut reader)?;
            Self::check_class_name(&cls, header.class_name_count)?;
            class_names.push(cls);
        }

        Ok(Self {
            objects,
            keys,
            values,
            class_names,
            format_version: header.format_version,
            coder_version: header.coder_version,
        })
    }

    fn check_object(obj: &Object, value_count: u32, class_name_count: u32) -> Result<(), Error> {
        if (obj.values_index() + obj.value_count()) as u32 > value_count {
            return Err(Error::FormatError("Value index out of bounds".into()));
        }
        if obj.class_name_index() as u32 > class_name_count {
            return Err(Error::FormatError("Class name index out of bounds".into()));
        }
        Ok(())
    }

    fn check_value(val: &Value, key_count: u32) -> Result<(), Error> {
        if val.key_index() as u32 > key_count {
            return Err(Error::FormatError("Key index out of bounds".into()));
        }
        Ok(())
    }

    fn check_class_name(cls: &ClassName, class_name_count: u32) -> Result<(), Error> {
        for index in cls.fallback_classes_indeces() {
            if *index as u32 > class_name_count {
                return Err(Error::FormatError(
                    "Class name (fallback class) index out of bounds".into(),
                ));
            }
        }
        Ok(())
    }

    /// Encodes the given archive and saves it to a file with a given path.
    pub fn to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), Error> {
        let file = File::create(path)?;
        let mut reader = BufWriter::new(file);
        self.to_writer(&mut reader)
    }

    /// Encodes the given archive and returns a vector of bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::with_capacity(1024));
        self.to_writer(&mut cursor).unwrap(); // should be safe since we're writing into a vector
        cursor.into_inner()
    }

    /// Encodes the given archive using a writer.
    pub fn to_writer<T: Write>(&self, writer: &mut T) -> Result<(), Error> {
        // Each objects contains 3 fields with up to 2 bytes VarInt
        let mut objects_bytes = Vec::with_capacity(self.objects.len() * 3 * 2);
        for obj in &self.objects {
            objects_bytes.append(&mut obj.to_bytes());
        }

        // Let's estimate the average key length as 16 symbols
        let mut keys_bytes = Vec::with_capacity(self.keys.len() * (16 + 2));
        for key in &self.keys {
            keys_bytes.append(&mut encode_var_int(key.len() as i32));
            keys_bytes.extend(key.as_bytes());
        }

        let mut values_bytes = Vec::with_capacity(self.values.len() * (8 + 2));
        for val in &self.values {
            values_bytes.append(&mut val.to_bytes());
        }

        let mut classes_bytes = Vec::with_capacity(self.class_names.len() * 16);
        for cls in &self.class_names {
            classes_bytes.append(&mut cls.to_bytes());
        }

        let header = Header {
            format_version: self.format_version,
            coder_version: self.coder_version,
            object_count: self.objects.len() as u32,
            offset_objects: 50,
            key_count: self.keys.len() as u32,
            offset_keys: (50 + objects_bytes.len()) as u32,
            value_count: self.values.len() as u32,
            offset_values: (50 + objects_bytes.len() + keys_bytes.len()) as u32,
            class_name_count: self.class_names.len() as u32,
            offset_class_names: (50 + objects_bytes.len() + keys_bytes.len() + values_bytes.len())
                as u32,
        };

        writer.write_all(MAGIC_BYTES)?;
        writer.write_all(&header.to_bytes())?;
        writer.write_all(&objects_bytes)?;
        writer.write_all(&keys_bytes)?;
        writer.write_all(&values_bytes)?;
        writer.write_all(&classes_bytes)?;
        writer.flush()?;

        Ok(())
    }

    /// Returns the format version of the given archive.
    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    /// Sets the format version of the given archive.
    pub fn set_format_version(&mut self, value: u32) {
        self.format_version = value;
    }

    /// Returns the coder version of the given archive.
    pub fn coder_version(&self) -> u32 {
        self.coder_version
    }

    /// Set the coder version of the given archive.
    pub fn set_coder_version(&mut self, value: u32) {
        self.coder_version = value;
    }

    /// Returns a reference to a vector of the archive's [objects](Object).
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    /// Sets the archive's objects.
    ///
    /// Returns an error if one of objects references to a value or a class name
    /// that is out of bounds.
    pub fn set_objects(&mut self, objects: Vec<Object>) -> Result<(), Error> {
        for obj in &objects {
            Self::check_object(obj, self.values.len() as u32, self.class_names.len() as u32)?;
        }
        self.objects = objects;
        Ok(())
    }

    /// Returns an array of the archive's keys.
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// Sets the archive's keys.
    pub fn set_keys(&mut self, keys: Vec<String>) {
        self.keys = keys;
    }

    /// Returns a reference to a vector of the archive's [values](Value).
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// Sets the archive's values.
    ///
    /// Returns an error if one of values references to a key that is out of bounds.
    pub fn set_values(&mut self, values: Vec<Value>) -> Result<(), Error> {
        for val in &values {
            Self::check_value(val, self.keys.len() as u32)?;
        }
        self.values = values;
        Ok(())
    }

    /// Returns a reference to a vector of the archive's [class names](ClassName).
    pub fn class_names(&self) -> &[ClassName] {
        &self.class_names
    }

    /// Sets the archive's class names.
    ///
    /// Returns an error if one of classes references to a fallback class that is out of bounds.
    pub fn set_class_names(&mut self, class_names: Vec<ClassName>) -> Result<(), Error> {
        for cls in &class_names {
            Self::check_class_name(cls, class_names.len() as u32)?;
        }
        self.class_names = class_names;
        Ok(())
    }

    /// Consumes itself and returns returns a unit of objects, keys, values and class names.
    pub fn into_inner(self) -> (Vec<Object>, Vec<String>, Vec<Value>, Vec<ClassName>) {
        (self.objects, self.keys, self.values, self.class_names)
    }
}

/// Decodes a variable integer ([more info](https://github.com/matsmattsson/nibsqueeze/blob/master/NibArchive.md#varint-coding))
/// into a regular i32.
fn decode_var_int<T: Read + Seek>(reader: &mut T) -> Result<VarInt, Error> {
    let mut result = 0;
    let mut shift = 0;
    loop {
        let mut current_byte = [0; 1];
        reader.read_exact(&mut current_byte)?;
        let current_byte = current_byte[0];
        result |= (current_byte as VarInt & 0x7F) << shift;
        shift += 7;
        if (current_byte & 128) != 0 {
            break;
        }
    }
    Ok(result)
}

/// Encodes an i32 into a variable integer bytes.
fn encode_var_int(mut value: VarInt) -> Vec<u8> {
    let mut number_of_bytes = 0;
    let mut _v = value;
    loop {
        number_of_bytes += 1;
        _v >>= 7;
        if _v == 0 {
            break;
        }
    }

    let mut offset = 0;
    let mut bytes = vec![0; number_of_bytes];

    while offset < number_of_bytes {
        let digit: u8 = (0x7f & value) as u8;
        value >>= 7;

        let is_last_digit = value == 0;

        bytes[offset] = digit | if is_last_digit { 0x80 } else { 0 };
        offset += 1;

        if is_last_digit {
            break;
        }
    }

    bytes
}
