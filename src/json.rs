use anyhow::anyhow;
use std::fmt::Display;

use crate::{NIBArchive, ValueVariant};

/// Convert a NIB archive to a JSON object.
pub fn nib_to_json(
    archive: NIBArchive,
) -> Result<serde_json::Map<String, serde_json::Value>, anyhow::Error> {
    let mut json = serde_json::Map::new();
    for object in archive.objects() {
        let class_name = object.class_name(&archive.class_names()).name();

        let mut object_json = serde_json::Map::new();

        for value in object.values(&archive.values()).into_iter() {
            let key = value.key(&archive.keys());
            let inner_value = value.value();

            let json_value: serde_json::Value = match inner_value {
                ValueVariant::Int8(number) => to_json_number(*number)?.into(),
                ValueVariant::Int16(number) => to_json_number(*number)?.into(),
                ValueVariant::Int32(number) => to_json_number(*number)?.into(),
                ValueVariant::Int64(number) => to_json_number(*number as f64)?.into(),
                ValueVariant::Float(number) => to_json_number(*number)?.into(),
                ValueVariant::Double(number) => to_json_number(*number)?.into(),
                ValueVariant::Bool(boolean) => serde_json::Value::Bool(*boolean).into(),
                ValueVariant::Data(data) => {
                    // check if the data is a string
                    if let Ok(string) = std::str::from_utf8(&data) {
                        serde_json::Value::String(string.to_string()).into()
                    } else {
                        serde_json::Value::Array(
                            data.iter()
                                .map(|byte| {
                                    to_json_number(*byte as f64)
                                        .expect("Error: Failed to convert byte to JSON number")
                                        .into()
                                })
                                .collect(),
                        )
                    }
                }
                ValueVariant::Nil => serde_json::Value::Null.into(),
                ValueVariant::ObjectRef(object_ref) => {
                    eprintln!("Ignoring object reference: {:?}", object_ref);
                    continue;
                }
            };

            object_json.insert(key.to_string(), json_value);
        }

        json.insert(
            class_name.to_string(),
            serde_json::Value::Object(object_json),
        );
    }

    return Ok(json);
}

fn to_json_number<NumberT: Into<f64> + Copy + Display>(
    number: NumberT,
) -> anyhow::Result<serde_json::Number> {
    serde_json::Number::from_f64(number.into())
        .ok_or_else(|| anyhow!("Error: Failed to convert number {} to JSON number", number))
}
