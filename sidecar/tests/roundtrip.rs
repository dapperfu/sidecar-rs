use sidecar::conventions::photo;
use sidecar::format::value::{Value, ValueKind};
use sidecar::SidecarDocument;
use std::io::Cursor;

#[test]
fn roundtrip_all_scalar_types() {
    let mut doc = SidecarDocument::new();
    doc.set("null_field", Value::Null).unwrap();
    doc.set("bool_field", Value::Bool(false)).unwrap();
    doc.set("i64_field", Value::I64(-42)).unwrap();
    doc.set("u64_field", Value::U64(42)).unwrap();
    doc.set("f32_field", Value::F32(1.5)).unwrap();
    doc.set("f64_field", Value::F64(0.008)).unwrap();
    doc.set("str_field", Value::String("hello".into())).unwrap();
    doc.set("bytes_field", Value::Bytes(vec![0, 1, 2])).unwrap();

    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();

    let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
    assert_eq!(decoded.get("f64_field"), Some(&Value::F64(0.008)));
    assert_eq!(
        decoded.get("str_field"),
        Some(&Value::String("hello".into()))
    );
}

#[test]
fn photography_convention_keys_roundtrip() {
    let mut doc = SidecarDocument::new();
    doc.set(photo::GPS_LATITUDE, Value::F64(37.7749)).unwrap();
    doc.set(photo::GPS_LONGITUDE, Value::F64(-122.4194))
        .unwrap();
    doc.set(photo::EXPOSURE_TIME_SEC, Value::F64(0.008))
        .unwrap();
    doc.set(photo::EXPOSURE_ISO, Value::U64(400)).unwrap();
    doc.set(photo::LENS_FOCAL_LENGTH_MM, Value::F32(50.0))
        .unwrap();
    doc.set(photo::RATING, Value::U64(4)).unwrap();
    doc.set(
        photo::TAGS,
        Value::Array {
            element_kind: ValueKind::String,
            elements: vec![Value::String("spring".into())],
        },
    )
    .unwrap();

    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();

    let decoded = SidecarDocument::from_reader(Cursor::new(buf)).unwrap();
    assert_eq!(decoded.get(photo::GPS_LATITUDE), Some(&Value::F64(37.7749)));
    assert_eq!(decoded.get(photo::EXPOSURE_ISO), Some(&Value::U64(400)));
    assert_eq!(
        decoded.get(photo::TAGS),
        Some(&Value::Array {
            element_kind: ValueKind::String,
            elements: vec![Value::String("spring".into())],
        })
    );
}

#[test]
fn remove_key() {
    let mut doc = SidecarDocument::new();
    doc.set("temp", Value::U64(1)).unwrap();
    assert_eq!(doc.remove("temp"), Some(Value::U64(1)));
    assert!(doc.get("temp").is_none());
}
