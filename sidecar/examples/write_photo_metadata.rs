//! Example: write photography metadata to a SCAR sidecar file.

use sidecar::conventions::photo;
use sidecar::format::value::{Value, ValueKind};
use sidecar::{sidecar_path_for_media, SidecarDocument};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let media = Path::new("photo.jpg");
    let sidecar_path = sidecar_path_for_media(media);

    let mut doc = SidecarDocument::new();
    doc.set_media_basename("photo.jpg")?;

    doc.set(photo::GPS_LATITUDE, Value::F64(37.7749))?;
    doc.set(photo::GPS_LONGITUDE, Value::F64(-122.4194))?;
    doc.set(photo::EXPOSURE_TIME_SEC, Value::F64(0.008))?;
    doc.set(photo::EXPOSURE_ISO, Value::U64(400))?;
    doc.set(photo::LENS_FOCAL_LENGTH_MM, Value::F32(50.0))?;
    doc.set(photo::RATING, Value::U64(4))?;
    doc.set(
        photo::TAGS,
        Value::Array {
            element_kind: ValueKind::String,
            elements: vec![
                Value::String("spring".into()),
                Value::String("outdoor".into()),
            ],
        },
    )?;

    doc.to_path(&sidecar_path)?;
    println!("Wrote {}", sidecar_path.display());
    Ok(())
}
