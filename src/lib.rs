#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

mod class_name;
mod error;
mod header;
mod object;
mod value;
pub use crate::{class_name::*, error::*, header::*, object::*, value::*};

use std::{
    fs::File,
    io::{BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write},
};

const MAGIC_BYTES: &[u8; 10] = b"NIBArchive";
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
    header: Header,
    objects: Vec<Object>,
    keys: Vec<String>,
    values: Vec<Value>,
    class_names: Vec<ClassName>,
}

impl NIBArchive {
    /// Creates a new NIB Archive from `objects`, `keys`, `values` and `class_names`.
    ///
    /// This method **does not** check the input values. For example, a situation when an object's
    /// key index is out of bounds of the `keys` parameter is unchecked. To ensure properly formed
    /// archive you should check those yourself.
    pub fn new(
        objects: Vec<Object>,
        keys: Vec<String>,
        values: Vec<Value>,
        class_names: Vec<ClassName>,
    ) -> Self {
        Self {
            header: Header::default(),
            objects,
            keys,
            values,
            class_names,
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
            if (obj.values_index() + obj.value_count()) as u32 > header.value_count {
                return Err(Error::FormatError("Value index out of bounds".into()));
            }
            if obj.class_name_index() as u32 > header.class_name_count {
                return Err(Error::FormatError("Class name index out of bounds".into()));
            }
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
            if val.key_index() as u32 > header.key_count {
                return Err(Error::FormatError("Key index out of bounds".into()));
            }
            values.push(val);
        }
        check_position!(reader, header.offset_class_names, "class names'");

        // Parse class names
        let mut class_names = Vec::with_capacity(header.class_name_count as usize);
        for _ in 0..header.class_name_count {
            let cls = ClassName::try_from_reader(&mut reader)?;
            for index in cls.fallback_classes_indeces() {
                if *index as u32 > header.class_name_count {
                    return Err(Error::FormatError(
                        "Class name (fallback class) index out of bounds".into(),
                    ));
                }
            }
            class_names.push(cls);
        }

        Ok(Self {
            header,
            objects,
            keys,
            values,
            class_names,
        })
    }

    /// Encodes an archive and saves it to a file with a given path.
    pub fn to_file<P: AsRef<std::path::Path>>(&mut self, path: P) -> Result<(), Error> {
        let file = File::create(path)?;
        let mut reader = BufWriter::new(file);
        self.to_writer(&mut reader)
    }

    /// Encodes an archive and returns a vector of bytes.
    pub fn to_bytes(&mut self) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::with_capacity(1024));
        self.to_writer(&mut cursor).unwrap(); // should be safe since we're writing into a vector
        cursor.into_inner()
    }

    /// Encodes an archive using a given writer.
    pub fn to_writer<T: Write>(&mut self, writer: &mut T) -> Result<(), Error> {
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
            format_version: self.header.format_version,
            coder_version: self.header.coder_version,
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
        self.header = header;

        Ok(())
    }

    /// Returns a reference to a [Header] that describes the current archive.
    ///
    /// If you're creating a new archive using the [new](Self::new()) method,
    /// the header is going to be empty, except for `format_version` and `coder_version`
    /// fields. Other fields will be filled after encoding your archive.
    ///
    /// If you read an archive and then change its properties,
    /// the header won't reflect these changes until the next encoding.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Set header's format version field.
    pub fn set_format_version(&mut self, value: u32) {
        self.header.format_version = value;
    }

    /// Set header's coder version field.
    pub fn set_coder_version(&mut self, value: u32) {
        self.header.coder_version = value;
    }

    /// Returns a reference to a vector of archive's [objects](Object).
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    /// Returns a mutable reference to a vector of archive's [objects](Object).
    pub fn objects_mut(&mut self) -> &mut Vec<Object> {
        &mut self.objects
    }

    /// Returns an array of archive's keys.
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// Returns a mutable reference to a vector of archive's keys.
    pub fn keys_mut(&mut self) -> &mut Vec<String> {
        &mut self.keys
    }

    /// Returns a reference to a vector of archive's [values](Value).
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// Returns a mutable reference to a vector of archive's [values](Value).
    pub fn values_mut(&mut self) -> &mut Vec<Value> {
        &mut self.values
    }

    /// Returns a reference to a vector of archive's [class names](ClassName).
    pub fn class_names(&self) -> &[ClassName] {
        &self.class_names
    }

    /// Returns a mutable reference to a vector of archive's [class names](ClassName).
    pub fn class_names_mut(&mut self) -> &mut Vec<ClassName> {
        &mut self.class_names
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
        let mut _current_byte = [0; 1];
        reader.read_exact(&mut _current_byte)?;
        let current_byte = _current_byte[0];
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
