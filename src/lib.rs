use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek, SeekFrom},
};

const MAGIC_BYTES: &[u8; 10] = b"NIBArchive";
type VarInt = i32;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IOError: {0}")]
    IOError(#[from] std::io::Error),
    #[error("NibArchive format error: {0}")]
    NibArchiveFormatError(String),
}

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

impl Header {
    pub fn from_bytes(bytes: [u8; 40]) -> Self {
        let mut values = [0; 10];
        for i in 0..10 {
            let int_bytes: [u8; 4] = [
                bytes[i * 4],
                bytes[i * 4 + 1],
                bytes[i * 4 + 2],
                bytes[i * 4 + 3],
            ];
            values[i] = u32::from_le_bytes(int_bytes);
        }
        Self {
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Object {
    class_name_index: VarInt,
    values_index: VarInt,
    value_count: VarInt,
}
impl Object {
    pub fn class_name_index(&self) -> VarInt {
        self.class_name_index
    }
    pub fn values_index(&self) -> VarInt {
        self.values_index
    }
    pub fn value_count(&self) -> VarInt {
        self.value_count
    }
}

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
            4 => ValueVariant::Bool(true),
            5 => ValueVariant::Bool(false),
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
                return Err(Error::NibArchiveFormatError(format!(
                    "unknown value type {value_type_byte:#04x}"
                )))
            }
        };
        Ok(Self { key_index, value })
    }

    pub fn key_index(&self) -> VarInt {
        self.key_index
    }
    pub fn value(&self) -> &ValueVariant {
        &self.value
    }
    pub fn into_inner(self) -> ValueVariant {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassName {
    name: String,
    fallback_classes: Vec<i32>,
}
impl ClassName {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn fallback_classes(&self) -> &[i32] {
        &self.fallback_classes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NibArchive {
    header: Header,
    objects: Vec<Object>,
    keys: Vec<String>,
    values: Vec<Value>,
    class_names: Vec<ClassName>,
}

macro_rules! check_position {
    ($reader:ident, $offset:expr, $err:literal) => {
        if $reader.stream_position()? != $offset as u64 {
            return Err(Error::NibArchiveFormatError(format!(
                "expected {} offset at {} - got {}",
                $err,
                $reader.stream_position()?,
                $offset
            )));
        }
    };
}

impl NibArchive {
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        Self::from_reader(&mut reader)
    }

    pub fn from_bytes<B: AsRef<[u8]>>(bytes: B) -> Result<Self, Error> {
        let mut cursor = Cursor::new(bytes);
        Self::from_reader(&mut cursor)
    }

    pub fn from_reader<T: Read + Seek>(mut reader: &mut T) -> Result<Self, Error> {
        reader.seek(SeekFrom::Start(0))?;

        // Check magic bytes
        let mut magic_bytes = [0; 10];
        reader.read_exact(&mut magic_bytes)?;
        if &magic_bytes != MAGIC_BYTES {
            return Err(Error::NibArchiveFormatError("magic bytes don't match".into()));
        }

        // Parse header
        let mut header_bytes = [0; 40];
        reader.read_exact(&mut header_bytes)?;
        let header = Header::from_bytes(header_bytes);

        check_position!(reader, header.offset_objects, "object");

        // Parse objects
        let mut objects = Vec::with_capacity(header.object_count as usize);
        for _ in 0..header.object_count {
            objects.push(Object {
                class_name_index: decode_var_int(&mut reader)?,
                values_index: decode_var_int(&mut reader)?,
                value_count: decode_var_int(&mut reader)?,
            });
        }
        check_position!(reader, header.offset_keys, "keys");

        // Parse keys
        let mut keys = Vec::with_capacity(header.key_count as usize);
        for _ in 0..header.key_count {
            let length = decode_var_int(&mut reader)?;
            let mut name_bytes = vec![0; length as usize];
            reader.read_exact(&mut name_bytes)?;
            let name = String::from_utf8(name_bytes).unwrap();
            keys.push(name);
        }
        check_position!(reader, header.offset_values, "values");

        // Parse values
        let mut values = Vec::with_capacity(header.value_count as usize);
        for _ in 0..header.value_count {
            values.push(Value::try_from_reader(&mut reader)?);
        }
        check_position!(reader, header.offset_class_names, "class names'");

        // Parse class names
        let mut class_names = Vec::with_capacity(header.class_name_count as usize);
        for _ in 0..header.class_name_count {
            let length = decode_var_int(&mut reader)?;
            let fallback_classes_count = decode_var_int(&mut reader)?;
            let mut fallback_classes = Vec::with_capacity(fallback_classes_count as usize);
            for _ in 0..fallback_classes_count {
                let mut buf = [0; 4];
                reader.read_exact(&mut buf)?;
                fallback_classes.push(i32::from_le_bytes(buf));
            }
            let mut name_bytes = vec![0; length as usize];
            reader.read_exact(&mut name_bytes)?;
            name_bytes.pop(); // Name is \0 terminated, so we have to remove the trailing \0
            let name = String::from_utf8(name_bytes).unwrap();

            class_names.push(ClassName {
                name,
                fallback_classes,
            });
        }

        Ok(Self {
            header,
            objects,
            keys,
            values,
            class_names,
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    pub fn values(&self) -> &[Value] {
        &self.values
    }

    pub fn class_names(&self) -> &[ClassName] {
        &self.class_names
    }
}

fn decode_var_int<T: Read + Seek>(reader: &mut T) -> Result<VarInt, Error> {
    let mut result = 0;
    let mut shift = 0;
    loop {
        let mut _current_byte = [0; 1];
        reader.read_exact(&mut _current_byte)?;
        let current_byte = _current_byte[0];
        result |= ((current_byte & 0x7F) << shift) as VarInt;
        shift += 7;
        if (current_byte & 128) != 0 {
            break;
        }
    }
    Ok(result)
}
