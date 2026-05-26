fn add_media_filter_payload_fields(payload: &mut Value, media: &ImagePayload) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    let Some(metadata) = &media.photo_metadata else {
        object.insert("photo_has_gps".to_string(), Value::Bool(false));
        return;
    };
    object.insert(
        "photo_has_gps".to_string(),
        Value::Bool(metadata.gps.is_some()),
    );
    if let Some(capture_time) = metadata
        .capture_time
        .as_deref()
        .and_then(parse_capture_time_epoch)
    {
        if let Some(number) = serde_json::Number::from_f64(capture_time) {
            object.insert(
                "photo_capture_time_epoch".to_string(),
                Value::Number(number),
            );
        }
    }
    let camera_text = [
        metadata.camera_make.as_deref(),
        metadata.camera_model.as_deref(),
        metadata.lens_model.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    if !camera_text.is_empty() {
        object.insert("photo_camera_text".to_string(), Value::String(camera_text));
    }
    if !metadata.keywords.is_empty() {
        object.insert(
            "photo_keywords".to_string(),
            Value::Array(
                metadata
                    .keywords
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
}

fn parse_capture_time_epoch(value: &str) -> Option<f64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| {
            datetime.timestamp() as f64
                + f64::from(datetime.timestamp_subsec_nanos()) / 1_000_000_000.0
        })
}
