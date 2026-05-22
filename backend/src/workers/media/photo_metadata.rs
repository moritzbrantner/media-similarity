use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use metastrip::{ExifValue, Metadata};

use crate::domain::models::{PhotoGpsPayload, PhotoMetadataEntryPayload, PhotoMetadataPayload};

pub fn extract_photo_metadata(path: &Path) -> Result<Option<PhotoMetadataPayload>, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let metadata = match metastrip::extract_metadata(&bytes) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    Ok(normalize_metadata(metadata))
}

fn normalize_metadata(metadata: Metadata) -> Option<PhotoMetadataPayload> {
    let mut payload = PhotoMetadataPayload::default();
    let mut keyword_keys = BTreeSet::new();

    if let Some(exif) = metadata.exif {
        let mut fields = exif.fields.into_iter().collect::<Vec<_>>();
        fields.sort_by(|left, right| left.0.cmp(&right.0));
        for (key, value) in fields {
            let value = exif_value_to_string(value);
            push_raw(&mut payload, "exif", &key, &key, &value);
            apply_common_field(&mut payload, &mut keyword_keys, "exif", &key, &value);
        }
    }

    if let Some(iptc) = metadata.iptc {
        let mut fields = iptc.fields.into_iter().collect::<Vec<_>>();
        fields.sort_by(|left, right| left.0.cmp(&right.0));
        for (key, value) in fields {
            push_raw(&mut payload, "iptc", &key, &key, &value);
            apply_common_field(&mut payload, &mut keyword_keys, "iptc", &key, &value);
        }
    }

    if let Some(xmp) = metadata.xmp {
        let raw_xml = xmp.raw_xml;
        push_raw(&mut payload, "xmp", "raw_xml", "Raw XML", &raw_xml);
        apply_xmp_fields(&mut payload, &mut keyword_keys, &raw_xml);
    }

    normalize_gps(&mut payload);

    if payload.capture_time.is_some()
        || payload.camera_make.is_some()
        || payload.camera_model.is_some()
        || payload.lens_model.is_some()
        || payload.orientation.is_some()
        || payload.gps.is_some()
        || payload.rating.is_some()
        || !payload.keywords.is_empty()
        || payload.title.is_some()
        || payload.description.is_some()
        || payload.creator.is_some()
        || payload.copyright.is_some()
        || !payload.raw.is_empty()
    {
        Some(payload)
    } else {
        None
    }
}

fn apply_common_field(
    payload: &mut PhotoMetadataPayload,
    keyword_keys: &mut BTreeSet<String>,
    namespace: &str,
    key: &str,
    value: &str,
) {
    let normalized_key = normalize_key(key);
    match normalized_key.as_str() {
        "datetimeoriginal" | "createdate" | "datecreated" | "creationdate" | "datetime" => {
            set_if_empty(&mut payload.capture_time, value)
        }
        "make" | "cameramake" => set_if_empty(&mut payload.camera_make, value),
        "model" | "cameramodel" => set_if_empty(&mut payload.camera_model, value),
        "lensmodel" | "lens" | "lensinfo" => set_if_empty(&mut payload.lens_model, value),
        "orientation" => set_if_empty(&mut payload.orientation, value),
        "rating" if payload.rating.is_none() => {
            payload.rating = parse_number(value).map(|number| number as f32);
        }
        "keywords" | "keyword" | "subject" => {
            add_keywords(&mut payload.keywords, keyword_keys, value)
        }
        "objectname" | "headline" | "title" => set_if_empty(&mut payload.title, value),
        "captionabstract" | "caption" | "description" | "imagedescription" => {
            set_if_empty(&mut payload.description, value)
        }
        "byline" | "creator" | "artist" => set_if_empty(&mut payload.creator, value),
        "copyrightnotice" | "copyright" => set_if_empty(&mut payload.copyright, value),
        "gpslatitude" | "gpslatituderef" | "gpslongitude" | "gpslongituderef" | "gpsaltitude" => {}
        _ if namespace == "xmp" => {}
        _ => {}
    }
}

fn apply_xmp_fields(
    payload: &mut PhotoMetadataPayload,
    keyword_keys: &mut BTreeSet<String>,
    raw_xml: &str,
) {
    for (key, aliases) in [
        (
            "CreateDate",
            &["CreateDate", "xmp:CreateDate", "photoshop:DateCreated"][..],
        ),
        ("Make", &["Make", "tiff:Make", "exif:Make"]),
        ("Model", &["Model", "tiff:Model", "exif:Model"]),
        ("LensModel", &["LensModel", "aux:Lens", "aux:LensInfo"]),
        ("Orientation", &["Orientation", "tiff:Orientation"]),
        ("Rating", &["Rating", "xmp:Rating"]),
        ("Creator", &["creator", "dc:creator"]),
        ("Title", &["title", "dc:title"]),
        ("Description", &["description", "dc:description"]),
        ("Copyright", &["rights", "dc:rights"]),
    ] {
        if let Some(value) = extract_xmp_value(raw_xml, aliases) {
            push_raw(payload, "xmp", key, key, &value);
            apply_common_field(payload, keyword_keys, "xmp", key, &value);
        }
    }

    for keyword in extract_xmp_list(raw_xml, &["subject", "dc:subject", "Keywords"]) {
        push_raw(payload, "xmp", "Keyword", "Keyword", &keyword);
        add_keyword(&mut payload.keywords, keyword_keys, &keyword);
    }

    let latitude = extract_xmp_value(raw_xml, &["GPSLatitude", "exif:GPSLatitude"]);
    let longitude = extract_xmp_value(raw_xml, &["GPSLongitude", "exif:GPSLongitude"]);
    let altitude = extract_xmp_value(raw_xml, &["GPSAltitude", "exif:GPSAltitude"]);
    if let (Some(latitude), Some(longitude)) = (latitude, longitude) {
        if let (Some(latitude), Some(longitude)) = (
            parse_coordinate(&latitude, None),
            parse_coordinate(&longitude, None),
        ) {
            payload.gps = Some(PhotoGpsPayload {
                latitude,
                longitude,
                altitude_meters: altitude.as_deref().and_then(parse_number),
            });
        }
    }
}

fn normalize_gps(payload: &mut PhotoMetadataPayload) {
    if payload.gps.is_some() {
        return;
    }
    let latitude = raw_value(payload, "exif", "GPSLatitude")
        .and_then(|value| parse_coordinate(value, raw_value(payload, "exif", "GPSLatitudeRef")));
    let longitude = raw_value(payload, "exif", "GPSLongitude")
        .and_then(|value| parse_coordinate(value, raw_value(payload, "exif", "GPSLongitudeRef")));
    if let (Some(latitude), Some(longitude)) = (latitude, longitude) {
        payload.gps = Some(PhotoGpsPayload {
            latitude,
            longitude,
            altitude_meters: raw_value(payload, "exif", "GPSAltitude").and_then(parse_number),
        });
    }
}

fn raw_value<'a>(payload: &'a PhotoMetadataPayload, namespace: &str, key: &str) -> Option<&'a str> {
    payload
        .raw
        .iter()
        .find(|entry| entry.namespace == namespace && entry.key.eq_ignore_ascii_case(key))
        .map(|entry| entry.value.as_str())
}

fn set_if_empty(target: &mut Option<String>, value: &str) {
    if target.is_some() {
        return;
    }
    let value = value.trim();
    if !value.is_empty() {
        *target = Some(value.to_string());
    }
}

fn add_keywords(keywords: &mut Vec<String>, seen: &mut BTreeSet<String>, value: &str) {
    for keyword in value
        .split([',', ';', '|'])
        .map(str::trim)
        .filter(|keyword| !keyword.is_empty())
    {
        add_keyword(keywords, seen, keyword);
    }
}

fn add_keyword(keywords: &mut Vec<String>, seen: &mut BTreeSet<String>, keyword: &str) {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return;
    }
    if seen.insert(keyword.to_ascii_lowercase()) {
        keywords.push(keyword.to_string());
    }
}

fn push_raw(
    payload: &mut PhotoMetadataPayload,
    namespace: &str,
    key: &str,
    label: &str,
    value: &str,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    payload.raw.push(PhotoMetadataEntryPayload {
        namespace: namespace.to_string(),
        key: key.to_string(),
        label: label.to_string(),
        value: value.to_string(),
    });
}

fn exif_value_to_string(value: ExifValue) -> String {
    match value {
        ExifValue::Text(value) => value,
        ExifValue::Unsigned(value) => value.to_string(),
        ExifValue::SignedRational(numerator, denominator) => {
            rational_to_string(numerator as f64, denominator as f64)
        }
        ExifValue::UnsignedRational(numerator, denominator) => {
            rational_to_string(numerator as f64, denominator as f64)
        }
        ExifValue::Binary(value) => format!("{} byte(s)", value.len()),
    }
}

fn rational_to_string(numerator: f64, denominator: f64) -> String {
    if denominator == 0.0 {
        numerator.to_string()
    } else {
        let value = numerator / denominator;
        if value.fract() == 0.0 {
            format!("{value:.0}")
        } else {
            value.to_string()
        }
    }
}

fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn parse_number(value: &str) -> Option<f64> {
    let value = value.trim();
    if let Some((numerator, denominator)) = value.split_once('/') {
        let numerator = numerator.trim().parse::<f64>().ok()?;
        let denominator = denominator.trim().parse::<f64>().ok()?;
        return (denominator != 0.0).then_some(numerator / denominator);
    }
    value.parse::<f64>().ok()
}

fn parse_coordinate(value: &str, reference: Option<&str>) -> Option<f64> {
    let mut parts = value
        .split(|character: char| {
            !(character.is_ascii_digit() || character == '.' || character == '-')
        })
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<f64>().ok());
    let first = parts.next()?;
    let second = parts.next();
    let third = parts.next();
    let mut decimal = first.abs();
    if let Some(minutes) = second {
        decimal += minutes / 60.0;
    }
    if let Some(seconds) = third {
        decimal += seconds / 3600.0;
    }
    if first.is_sign_negative() || matches!(reference, Some("S" | "s" | "W" | "w")) {
        decimal = -decimal;
    }
    Some(decimal)
}

fn extract_xmp_value(raw_xml: &str, names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(value) = extract_xml_attribute(raw_xml, name) {
            return Some(value);
        }
        if let Some(value) = extract_xml_text(raw_xml, name) {
            return Some(value);
        }
    }
    None
}

fn extract_xml_attribute(raw_xml: &str, name: &str) -> Option<String> {
    let pattern = format!("{name}=\"");
    let start = raw_xml.find(&pattern)? + pattern.len();
    let end = raw_xml[start..].find('"')? + start;
    clean_xml_text(&raw_xml[start..end])
}

fn extract_xml_text(raw_xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = raw_xml.find(&open)? + open.len();
    let end = raw_xml[start..].find(&close)? + start;
    let inner = &raw_xml[start..end];
    extract_rdf_text(inner).or_else(|| clean_xml_text(inner))
}

fn extract_rdf_text(raw_xml: &str) -> Option<String> {
    extract_xml_text(raw_xml, "rdf:li")
}

fn extract_xmp_list(raw_xml: &str, names: &[&str]) -> Vec<String> {
    let mut values = Vec::new();
    for name in names {
        let open = format!("<{name}>");
        let close = format!("</{name}>");
        let Some(start) = raw_xml.find(&open).map(|index| index + open.len()) else {
            continue;
        };
        let Some(end) = raw_xml[start..].find(&close).map(|index| index + start) else {
            continue;
        };
        values.extend(extract_all_xml_text(&raw_xml[start..end], "rdf:li"));
    }
    values
}

fn extract_all_xml_text(raw_xml: &str, name: &str) -> Vec<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let mut remaining = raw_xml;
    let mut values = Vec::new();
    while let Some(start) = remaining.find(&open).map(|index| index + open.len()) {
        let Some(end) = remaining[start..].find(&close).map(|index| index + start) else {
            break;
        };
        if let Some(value) = clean_xml_text(&remaining[start..end]) {
            values.push(value);
        }
        remaining = &remaining[end + close.len()..];
    }
    values
}

fn clean_xml_text(value: &str) -> Option<String> {
    let value = value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{add_keyword, extract_photo_metadata, parse_coordinate};

    #[test]
    fn no_metadata_returns_none() {
        let dir = tempfile_dir();
        let path = dir.join("empty.jpg");
        fs::write(&path, [0xff, 0xd8, 0xff, 0xd9]).unwrap();

        assert_eq!(extract_photo_metadata(&path).unwrap(), None);
    }

    #[test]
    fn malformed_metadata_does_not_panic() {
        let dir = tempfile_dir();
        let path = dir.join("broken.jpg");
        fs::write(&path, [0xff, 0xd8, 0xff, 0xe1, 0x00]).unwrap();

        assert_eq!(extract_photo_metadata(&path).unwrap(), None);
    }

    #[test]
    fn xmp_common_fields_are_normalized() {
        let dir = tempfile_dir();
        let path = dir.join("photo.jpg");
        fs::write(&path, jpeg_with_xmp(test_xmp())).unwrap();

        let metadata = extract_photo_metadata(&path).unwrap().unwrap();

        assert_eq!(
            metadata.capture_time.as_deref(),
            Some("2024-03-12T10:30:00Z")
        );
        assert_eq!(metadata.camera_make.as_deref(), Some("Acme"));
        assert_eq!(metadata.camera_model.as_deref(), Some("Pocket 7"));
        assert_eq!(metadata.lens_model.as_deref(), Some("35mm Prime"));
        assert_eq!(metadata.rating, Some(4.0));
        assert_eq!(metadata.keywords, vec!["Travel", "Sunrise"]);
        assert_eq!(metadata.gps.unwrap().latitude, 52.5);
    }

    #[test]
    fn keyword_dedupe_is_case_insensitive() {
        let mut keywords = Vec::new();
        let mut seen = Default::default();
        add_keyword(&mut keywords, &mut seen, "Travel");
        add_keyword(&mut keywords, &mut seen, "travel");
        add_keyword(&mut keywords, &mut seen, "Sunrise");

        assert_eq!(keywords, vec!["Travel", "Sunrise"]);
    }

    #[test]
    fn gps_requires_latitude_and_longitude_at_payload_level() {
        assert_eq!(parse_coordinate("52 30 0", Some("N")), Some(52.5));
        assert_eq!(parse_coordinate("13 24 0", Some("W")), Some(-13.4));
    }

    fn test_xmp() -> &'static str {
        r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:tiff="http://ns.adobe.com/tiff/1.0/" xmlns:aux="http://ns.adobe.com/exif/1.0/aux/" xmlns:exif="http://ns.adobe.com/exif/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/">
<rdf:Description xmp:CreateDate="2024-03-12T10:30:00Z" tiff:Make="Acme" tiff:Model="Pocket 7" aux:Lens="35mm Prime" xmp:Rating="4" exif:GPSLatitude="52.5" exif:GPSLongitude="13.4">
<dc:subject><rdf:Bag><rdf:li>Travel</rdf:li><rdf:li>travel</rdf:li><rdf:li>Sunrise</rdf:li></rdf:Bag></dc:subject>
</rdf:Description>
</rdf:RDF>
</x:xmpmeta>"#
    }

    fn jpeg_with_xmp(xmp: &str) -> Vec<u8> {
        let mut payload = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
        payload.extend_from_slice(xmp.as_bytes());
        let length = payload.len() + 2;
        let mut bytes = vec![0xff, 0xd8, 0xff, 0xe1];
        bytes.extend_from_slice(&(length as u16).to_be_bytes());
        bytes.extend_from_slice(&payload);
        bytes.extend_from_slice(&[0xff, 0xd9]);
        bytes
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("image-sim-photo-meta-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
