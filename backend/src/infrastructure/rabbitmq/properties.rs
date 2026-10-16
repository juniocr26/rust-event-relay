//! Lossless client-model properties v1; binary base64 and IEEE float bits.
//! Propriedades v1 sem perda no modelo do cliente; base64 e bits IEEE.
use crate::application::consumer::QuarantineMessage;
use base64::{Engine, engine::general_purpose::STANDARD};
use lapin::{
    BasicProperties,
    types::{AMQPValue, FieldTable},
};
use serde_json::{Value, json};

struct Budget {
    nodes: usize,
    bytes: usize,
}
impl Budget {
    fn charge(&mut self, depth: usize, bytes: usize) -> Option<()> {
        self.nodes = self.nodes.checked_sub(1)?;
        self.bytes = self.bytes.checked_sub(bytes)?;
        (depth <= 16).then_some(())
    }
}
fn encode(v: &AMQPValue, depth: usize, budget: &mut Budget) -> Option<Value> {
    use AMQPValue::*;
    budget.charge(depth, 0)?;
    Some(match v {
        Float(f) => json!({"FloatBits": f.to_bits()}),
        Double(f) => json!({"DoubleBits": f.to_bits()}),
        LongString(s) => {
            budget.charge(depth, s.len())?;
            json!({"LongStringBase64": STANDARD.encode(s.as_bytes())})
        }
        ByteArray(b) => {
            budget.charge(depth, b.len())?;
            json!({"ByteArrayBase64": STANDARD.encode(b.as_slice())})
        }
        ShortString(s) => {
            if s.as_str().len() > 255 {
                return None;
            }
            budget.charge(depth, s.as_str().len())?;
            serde_json::to_value(v).ok()?
        }
        FieldArray(a) => {
            json!({"FieldArray": a.as_slice().iter().map(|v| encode(v, depth+1, budget)).collect::<Option<Vec<_>>>()?})
        }
        FieldTable(t) => json!({"FieldTable": table(t, depth+1, budget)?}),
        _ => serde_json::to_value(v).ok()?,
    })
}
fn table(t: &FieldTable, depth: usize, budget: &mut Budget) -> Option<Value> {
    let mut out = serde_json::Map::new();
    for (key, value) in t {
        budget.charge(depth, key.as_str().len())?;
        out.insert(key.as_str().into(), encode(value, depth, budget)?);
    }
    Some(Value::Object(out))
}
/// Bounded recursive encoding; no lossy UTF-8 or JSON float conversion.
/// Codificação recursiva limitada; sem conversão UTF-8 ou float com perda.
pub fn encode_properties(properties: &BasicProperties) -> Option<Value> {
    let mut budget = Budget {
        nodes: 512,
        bytes: QuarantineMessage::MAX_PROPERTIES_BYTES,
    };
    let headers = match properties.headers() {
        Some(headers) => table(headers, 0, &mut budget)?,
        None => Value::Null,
    };
    // Clone only after bounded traversal; clear headers to avoid float normalization.
    let mut out =
        serde_json::to_value(properties.clone().with_headers(FieldTable::default())).ok()?;
    if out
        .as_object()?
        .iter()
        .any(|(key, value)| key != "headers" && value.as_str().is_some_and(|s| s.len() > 255))
    {
        return None;
    }
    out["headers"] = headers;
    (serde_json::to_vec(&out).ok()?.len() <= QuarantineMessage::MAX_PROPERTIES_BYTES).then_some(out)
}
fn decode(v: &Value, depth: usize, budget: &mut Budget) -> Option<AMQPValue> {
    budget.charge(depth, 0)?;
    if let Some(tag) = v.as_object() {
        if tag.len() != 1 {
            return None;
        }
        let (key, value) = tag.iter().next()?;
        return Some(match key.as_str() {
            "FloatBits" => AMQPValue::Float(f32::from_bits(u32::try_from(value.as_u64()?).ok()?)),
            "DoubleBits" => AMQPValue::Double(f64::from_bits(value.as_u64()?)),
            "LongStringBase64" => {
                AMQPValue::LongString(STANDARD.decode(value.as_str()?).ok()?.into())
            }
            "ByteArrayBase64" => {
                AMQPValue::ByteArray(STANDARD.decode(value.as_str()?).ok()?.into())
            }
            "FieldArray" => AMQPValue::FieldArray(
                value
                    .as_array()?
                    .iter()
                    .map(|v| decode(v, depth + 1, budget))
                    .collect::<Option<Vec<_>>>()?
                    .into(),
            ),
            "FieldTable" => AMQPValue::FieldTable(decode_table(value, depth + 1, budget)?),
            _ => serde_json::from_value(v.clone()).ok()?,
        });
    }
    serde_json::from_value(v.clone()).ok()
}
fn decode_table(v: &Value, depth: usize, budget: &mut Budget) -> Option<FieldTable> {
    let mut out = FieldTable::default();
    for (key, value) in v.as_object()? {
        budget.charge(depth, key.len())?;
        if key.len() > 255 {
            return None;
        }
        out.insert(key.clone().into(), decode(value, depth, budget)?);
    }
    Some(out)
}
// Bound caller-provided JSON before cloning or serializing it.
fn json_bounds(value: &Value, depth: usize, nodes: &mut usize, bytes: &mut usize) -> Option<()> {
    *nodes = nodes.checked_sub(1)?;
    if depth > 64 {
        return None;
    }
    match value {
        Value::String(s) => {
            *bytes = bytes.checked_sub(s.len())?;
        }
        Value::Array(a) => {
            for item in a {
                json_bounds(item, depth + 1, nodes, bytes)?;
            }
        }
        Value::Object(o) => {
            for (key, item) in o {
                *bytes = bytes.checked_sub(key.len())?;
                json_bounds(item, depth + 1, nodes, bytes)?;
            }
        }
        _ => {}
    }
    Some(())
}
/// Checked restoration for inspection, not authority to replay a message.
/// Restauração verificada para inspeção, não autoriza replay.
pub fn decode_properties(value: &Value) -> Option<BasicProperties> {
    let mut nodes = 4096;
    let mut bytes = QuarantineMessage::MAX_PROPERTIES_BYTES;
    json_bounds(value, 0, &mut nodes, &mut bytes)?;
    if serde_json::to_vec(value).ok()?.len() > QuarantineMessage::MAX_PROPERTIES_BYTES {
        return None;
    }
    let headers = value.get("headers")?;
    let mut plain = value.clone();
    plain["headers"] = Value::Null;
    let mut out: BasicProperties = serde_json::from_value(plain).ok()?;
    if !headers.is_null() {
        out = out.with_headers(decode_table(
            headers,
            0,
            &mut Budget {
                nodes: 512,
                bytes: QuarantineMessage::MAX_PROPERTIES_BYTES,
            },
        )?);
    }
    // Reject alternative/unknown tags, omitted fields and unsupported normalization.
    (encode_properties(&out)? == *value).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_binary_numeric_types_and_nonfinite_bits_with_collision_isolation() {
        let mut headers = FieldTable::default();
        let values = vec![
            AMQPValue::Boolean(true),
            AMQPValue::ShortShortInt(-1),
            AMQPValue::ShortShortUInt(255),
            AMQPValue::ShortInt(-123),
            AMQPValue::ShortUInt(123),
            AMQPValue::LongInt(-456),
            AMQPValue::LongUInt(456),
            AMQPValue::LongLongInt(i64::MAX),
            AMQPValue::Timestamp(u64::MAX),
            AMQPValue::Float(f32::from_bits(0x7fc00001)),
            AMQPValue::Double(-0.0),
            AMQPValue::Double(f64::INFINITY),
            AMQPValue::DecimalValue(lapin::types::DecimalValue {
                scale: 3,
                value: 123,
            }),
            AMQPValue::LongString(vec![0, 255, 128].into()),
            AMQPValue::ByteArray(vec![255, 0].into()),
            AMQPValue::Void,
        ];
        headers.insert(
            "relay-preservation-v1".into(),
            AMQPValue::FieldArray(values.into()),
        );
        headers.insert(
            "nested".into(),
            AMQPValue::FieldTable(FieldTable::default()),
        );
        let original = BasicProperties::default()
            .with_headers(headers)
            .with_message_id("malformed".into())
            .with_expiration("1000".into())
            .with_delivery_mode(1)
            .with_timestamp(u64::MAX);
        let encoded = encode_properties(&original).unwrap();
        let restored = decode_properties(&encoded).unwrap();
        // NaN is not equal to itself; compare its retained bit representation instead.
        assert_eq!(encode_properties(&restored).unwrap(), encoded);
        assert_eq!(restored.expiration(), original.expiration());
        assert_eq!(
            encoded["headers"]["relay-preservation-v1"]["FieldArray"][9]["FloatBits"],
            0x7fc00001_u32
        );
        let absent = encode_properties(&BasicProperties::default()).unwrap();
        assert_eq!(absent["headers"], Value::Null);
        assert!(decode_properties(&serde_json::json!({})).is_none());
    }
    #[test]
    fn refuses_size_depth_and_node_excess_without_normalization() {
        let mut headers = FieldTable::default();
        headers.insert("large".into(), AMQPValue::LongString(vec![0; 16385].into()));
        assert!(encode_properties(&BasicProperties::default().with_headers(headers)).is_none());
        let mut value = AMQPValue::Void;
        for _ in 0..18 {
            value = AMQPValue::FieldArray(vec![value].into());
        }
        let mut headers = FieldTable::default();
        headers.insert("deep".into(), value);
        assert!(encode_properties(&BasicProperties::default().with_headers(headers)).is_none());
        let mut headers = FieldTable::default();
        headers.insert(
            "many".into(),
            AMQPValue::FieldArray(vec![AMQPValue::Void; 513].into()),
        );
        assert!(encode_properties(&BasicProperties::default().with_headers(headers)).is_none());
    }
}
