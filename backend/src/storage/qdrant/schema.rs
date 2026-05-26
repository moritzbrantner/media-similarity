#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PayloadFieldSchema {
    Keyword,
    #[serde(rename = "bool")]
    Bool,
    Integer,
    Float,
}

impl fmt::Display for PayloadFieldSchema {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Keyword => "keyword",
            Self::Bool => "bool",
            Self::Integer => "integer",
            Self::Float => "float",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PayloadIndexSpec {
    field_name: &'static str,
    field_schema: PayloadFieldSchema,
}

fn required_payload_indexes() -> &'static [PayloadIndexSpec] {
    &[
        PayloadIndexSpec {
            field_name: "point_kind",
            field_schema: PayloadFieldSchema::Keyword,
        },
        PayloadIndexSpec {
            field_name: "id",
            field_schema: PayloadFieldSchema::Keyword,
        },
        PayloadIndexSpec {
            field_name: "source_uri",
            field_schema: PayloadFieldSchema::Keyword,
        },
        PayloadIndexSpec {
            field_name: "source_item_uri",
            field_schema: PayloadFieldSchema::Keyword,
        },
        PayloadIndexSpec {
            field_name: "source_type",
            field_schema: PayloadFieldSchema::Keyword,
        },
        PayloadIndexSpec {
            field_name: "media_kind",
            field_schema: PayloadFieldSchema::Keyword,
        },
        PayloadIndexSpec {
            field_name: "photo_has_gps",
            field_schema: PayloadFieldSchema::Bool,
        },
        PayloadIndexSpec {
            field_name: "width",
            field_schema: PayloadFieldSchema::Integer,
        },
        PayloadIndexSpec {
            field_name: "height",
            field_schema: PayloadFieldSchema::Integer,
        },
        PayloadIndexSpec {
            field_name: "size_bytes",
            field_schema: PayloadFieldSchema::Integer,
        },
        PayloadIndexSpec {
            field_name: "modified_at",
            field_schema: PayloadFieldSchema::Float,
        },
        PayloadIndexSpec {
            field_name: "photo_capture_time_epoch",
            field_schema: PayloadFieldSchema::Float,
        },
    ]
}

fn payload_index_type<'a>(payload_schema: Option<&'a Value>, field_name: &str) -> Option<&'a str> {
    let value = payload_schema?.get(field_name)?;
    value
        .as_str()
        .or_else(|| value.get("data_type").and_then(Value::as_str))
        .or_else(|| value.get("type").and_then(Value::as_str))
}

fn payload_index_type_matches(actual: &str, expected: PayloadFieldSchema) -> bool {
    match expected {
        PayloadFieldSchema::Keyword => actual.eq_ignore_ascii_case("keyword"),
        PayloadFieldSchema::Bool => {
            actual.eq_ignore_ascii_case("bool") || actual.eq_ignore_ascii_case("boolean")
        }
        PayloadFieldSchema::Integer => actual.eq_ignore_ascii_case("integer"),
        PayloadFieldSchema::Float => actual.eq_ignore_ascii_case("float"),
    }
}

fn payload_index_schema_error(
    collection: &str,
    field_name: &str,
    expected: PayloadFieldSchema,
    actual: &str,
) -> String {
    format!(
        "Qdrant collection `{collection}` payload index `{field_name}` is incompatible with this service: expected {expected}, found {actual}. Delete and recreate the payload index, or set QDRANT_COLLECTION to a new empty collection name and re-index media."
    )
}

#[derive(Clone, Serialize)]
struct Filter {
    must: Vec<FieldCondition>,
}

#[derive(Clone, Serialize)]
struct FieldCondition {
    key: String,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    condition_match: Option<MatchValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<RangeCondition>,
}

#[derive(Clone, Serialize)]
struct RangeCondition {
    #[serde(skip_serializing_if = "Option::is_none")]
    gte: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lte: Option<f64>,
}

#[derive(Clone, Serialize)]
struct MatchValue {
    value: Value,
}

fn kind_filter(kind: &'static str) -> Filter {
    Filter {
        must: vec![field_condition("point_kind", kind)],
    }
}

fn media_search_filter(filter: Option<MediaSearchFilter>) -> Option<Filter> {
    let mut conditions = vec![field_condition("point_kind", "media")];
    let Some(filter) = filter else {
        return Some(Filter { must: conditions });
    };
    if let Some(source_type) = filter.source_type {
        conditions.push(field_condition("source_type", source_type));
    }
    if let Some(media_kind) = filter.media_kind {
        conditions.push(field_condition("media_kind", media_kind));
    }
    if let Some(has_gps) = filter.has_gps {
        conditions.push(bool_field_condition("photo_has_gps", has_gps));
    }
    push_range(
        &mut conditions,
        "width",
        filter.min_width.map(f64::from),
        filter.max_width.map(f64::from),
    );
    push_range(
        &mut conditions,
        "height",
        filter.min_height.map(f64::from),
        filter.max_height.map(f64::from),
    );
    push_range(
        &mut conditions,
        "size_bytes",
        filter.min_size_bytes.map(|value| value as f64),
        filter.max_size_bytes.map(|value| value as f64),
    );
    push_range(
        &mut conditions,
        "modified_at",
        filter.modified_from,
        filter.modified_to,
    );
    push_range(
        &mut conditions,
        "photo_capture_time_epoch",
        filter.captured_from,
        filter.captured_to,
    );
    Some(Filter { must: conditions })
}

fn push_range(
    conditions: &mut Vec<FieldCondition>,
    key: &'static str,
    gte: Option<f64>,
    lte: Option<f64>,
) {
    if gte.is_some() || lte.is_some() {
        conditions.push(range_field_condition(key, gte, lte));
    }
}

fn field_condition(key: impl Into<String>, value: impl Into<String>) -> FieldCondition {
    FieldCondition {
        key: key.into(),
        condition_match: Some(MatchValue {
            value: Value::String(value.into()),
        }),
        range: None,
    }
}

fn bool_field_condition(key: impl Into<String>, value: bool) -> FieldCondition {
    FieldCondition {
        key: key.into(),
        condition_match: Some(MatchValue {
            value: Value::Bool(value),
        }),
        range: None,
    }
}

fn range_field_condition(
    key: impl Into<String>,
    gte: Option<f64>,
    lte: Option<f64>,
) -> FieldCondition {
    FieldCondition {
        key: key.into(),
        condition_match: None,
        range: Some(RangeCondition { gte, lte }),
    }
}

fn set_payload_kind(payload: &mut Value, kind: &'static str) {
    if let Some(object) = payload.as_object_mut() {
        object.insert("point_kind".to_string(), Value::String(kind.to_string()));
    }
}
