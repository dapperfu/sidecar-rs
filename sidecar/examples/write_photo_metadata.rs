//! Example: write photography metadata to a CBOR sidecar file.

use sidecar::conventions::photo;
use sidecar::{sidecar_path_for_media, SidecarDocument, Value};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let media = Path::new("photo.jpg");
    let sidecar_path = sidecar_path_for_media(media);

    let mut doc = SidecarDocument::new();
    doc.set_media_basename("photo.jpg")?;

    doc.set(photo::GPS_LATITUDE, Value::Float(37.7749))?;
    doc.set(photo::GPS_LONGITUDE, Value::Float(-122.4194))?;
    doc.set(photo::EXPOSURE_TIME_SEC, Value::Float(0.008))?;
    doc.set(photo::EXPOSURE_ISO, Value::Integer(400))?;
    doc.set(photo::LENS_FOCAL_LENGTH_MM, Value::Float(50.0))?;
    doc.set(photo::RATING, Value::Integer(4))?;
    doc.set(
        photo::TAGS,
        Value::Array(vec![
            Value::Text("spring".into()),
            Value::Text("outdoor".into()),
        ]),
    )?;

    doc.to_path(&sidecar_path)?;
    println!("Wrote {}", sidecar_path.display());
    Ok(())
}
