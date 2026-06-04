use sidecar::conventions::photo;
use sidecar::{SidecarDocument, Value};
use std::io::Cursor;

#[test]
fn roundtrip_all_scalar_types() {
    let mut doc = SidecarDocument::new();
    doc.set("null_field", Value::Null).unwrap();
    doc.set("bool_field", Value::Bool(false)).unwrap();
    doc.set("i64_field", Value::Integer(-42)).unwrap();
    doc.set("u64_field", Value::Integer(42)).unwrap();
    doc.set("f32_field", Value::Float(1.5)).unwrap();
    doc.set("f64_field", Value::Float(0.008)).unwrap();
    doc.set("str_field", Value::Text("hello".into())).unwrap();
    doc.set("bytes_field", Value::Bytes(vec![0, 1, 2])).unwrap();

    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();

    let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
    assert_eq!(decoded.get("f64_field"), Some(&Value::Float(0.008)));
    assert_eq!(decoded.get("str_field"), Some(&Value::Text("hello".into())));
}

#[test]
fn photography_convention_keys_roundtrip() {
    let mut doc = SidecarDocument::new();
    doc.set(photo::GPS_LATITUDE, Value::Float(37.7749)).unwrap();
    doc.set(photo::GPS_LONGITUDE, Value::Float(-122.4194))
        .unwrap();
    doc.set(photo::EXPOSURE_TIME_SEC, Value::Float(0.008))
        .unwrap();
    doc.set(photo::EXPOSURE_ISO, Value::Integer(400)).unwrap();
    doc.set(photo::LENS_FOCAL_LENGTH_MM, Value::Float(50.0))
        .unwrap();
    doc.set(photo::RATING, Value::Integer(4)).unwrap();
    doc.set(
        photo::TAGS,
        Value::Array(vec![Value::Text("spring".into())]),
    )
    .unwrap();

    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();

    let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
    assert_eq!(
        decoded.get(photo::GPS_LATITUDE),
        Some(&Value::Float(37.7749))
    );
    assert_eq!(decoded.get(photo::EXPOSURE_ISO), Some(&Value::Integer(400)));
    assert_eq!(
        decoded.get(photo::TAGS),
        Some(&Value::Array(vec![Value::Text("spring".into())]))
    );
}

#[test]
fn remove_key() {
    let mut doc = SidecarDocument::new();
    doc.set("temp", Value::Integer(1)).unwrap();
    assert_eq!(doc.remove("temp"), Some(Value::Integer(1)));
    assert!(doc.get("temp").is_none());
}

#[test]
fn nested_map_roundtrip() {
    let mut inner = indexmap::IndexMap::new();
    inner.insert("enabled".into(), Value::Bool(true));
    inner.insert("level".into(), Value::Integer(3));

    let mut doc = SidecarDocument::new();
    doc.set("config", Value::Map(inner)).unwrap();

    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();

    let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
    assert_eq!(doc.get("config"), decoded.get("config"));
}

#[test]
fn heterogeneous_array_roundtrip() {
    let mut doc = SidecarDocument::new();
    doc.set(
        "mixed",
        Value::Array(vec![
            Value::Integer(1),
            Value::Text("two".into()),
            Value::Float(3.0),
        ]),
    )
    .unwrap();

    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();

    let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
    assert_eq!(doc.get("mixed"), decoded.get("mixed"));
}

#[test]
fn output_is_valid_cbor() {
    let mut doc = SidecarDocument::new();
    doc.set("k", Value::Integer(1)).unwrap();
    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();
    let cbor: ciborium::value::Value = ciborium::from_reader(Cursor::new(buf)).expect("valid CBOR");
    assert!(matches!(cbor, ciborium::value::Value::Map(_)));
}
