#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

use std::{
    fs::File,
    io::{BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write},
};

const MAGIC_BYTES: &[u8; 10] = b"NIBArchive";
type VarInt = i32;

/// Variants of error that may occur during encoding/decoding a NIB Archive.
#[derive(Debug)]
pub enum Error {
    /// An IO error that may occur during opening/reading/writing a file.
    IOError(std::io::Error),

    /// A format error that may occur only during decoding a NIB Archive.
    /// Usually it indicates a malformed file.
    FormatError(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IOError(e) => f.write_fmt(format_args!("IOError: {e}")),
            Error::FormatError(e) => f.write_fmt(format_args!("NIB Archive format error: {e}")),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IOError(value)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::FormatError(format!("Unable to parse UTF-8 string. {value}"))
    }
}

/// A NIB Archive header.
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
        Self { class_name_index, values_index, value_count }
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
    /// Pass the return value of [NIBArchive::values()] method for a proper result.
    pub fn values<'a>(&self, values: &'a [Value]) -> &'a [Value] {
        let start = self.values_index() as usize;
        let end = start + self.value_count() as usize;
        &values[start..end]
    }

    /// Returns a reference to a [ClassName] associated with the current object.
    ///
    /// Pass the return value of [NIBArchive::class_names()] method for a proper result.
    pub fn class_name<'a>(&self, class_names: &'a [ClassName]) -> &'a ClassName {
        &class_names[self.class_name_index() as usize]
    }

    /// Consumes itself and returns a unit of `class_name_index`, `values_index` and `value_count`.
    pub fn into_inner(self) -> (VarInt, VarInt, VarInt) {
        (self.class_name_index, self.values_index, self.value_count)
    }
}

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
        Self { name, fallback_classes_indeces }
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

    /// Returns a slice of [ClassNames](ClassName) representing fallback classes.
    ///
    /// Pass the return value of [NIBArchive::class_names()] method for a proper result.
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

/// After reading the current block of data it ensures that the current stream
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

/// NIB Archive encoder/decoder.
///
/// Look at module docs for more info.
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
            if (obj.values_index + obj.value_count) as u32 > header.value_count {
                return Err(Error::FormatError("Value index out of bounds".into()));
            }
            if obj.class_name_index as u32 > header.class_name_count {
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
            if val.key_index as u32 > header.key_count {
                return Err(Error::FormatError("Key index out of bounds".into()));
            }
            values.push(val);
        }
        check_position!(reader, header.offset_class_names, "class names'");

        // Parse class names
        let mut class_names = Vec::with_capacity(header.class_name_count as usize);
        for _ in 0..header.class_name_count {
            let cls = ClassName::try_from_reader(&mut reader)?;
            for index in &cls.fallback_classes_indeces {
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
        let mut cursor = Cursor::new( Vec::with_capacity(1024) );
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
            keys_bytes.extend_from_slice(key.as_bytes());
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
    /// If you're creating a new archive with the [new](Self::new()) method,
    /// the header is going to be empty, except for `format_version` and `coder_version`
    /// fields. It will be filled after encoding your archive.
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

    /// Returns a reference to a vector of archive's keys.
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
