#[cfg(test)]
mod tests {
    use reqwest::StatusCode;
    use serde_json::json;

    use super::{
        is_retryable_status, media_search_filter, payload_index_schema_error, payload_index_type,
        payload_index_type_matches, qdrant_base_urls, required_payload_indexes, retry_delay_ms,
        validate_collection_vectors, PayloadFieldSchema,
    };
    use crate::storage::MediaSearchFilter;

    #[test]
    fn qdrant_service_hostname_falls_back_to_host_port() {
        assert_eq!(
            qdrant_base_urls("http://qdrant:6333/"),
            vec!["http://qdrant:6333", "http://127.0.0.1:6333"]
        );
    }

    #[test]
    fn non_compose_qdrant_urls_are_left_alone() {
        assert_eq!(
            qdrant_base_urls("http://localhost:6333"),
            vec!["http://localhost:6333"]
        );
        assert_eq!(
            qdrant_base_urls("http://qdrant.internal:6333"),
            vec!["http://qdrant.internal:6333"]
        );
    }

    #[test]
    fn matching_collection_schema_is_valid() {
        let vectors = json!({
            "visual": { "size": 512, "distance": "Cosine" },
            "face": { "size": 256, "distance": "Cosine" }
        });

        assert!(validate_collection_vectors("images", 512, 256, &vectors).is_ok());
    }

    #[test]
    fn mismatched_collection_schema_reports_remediation() {
        let vectors = json!({
            "visual": { "size": 384, "distance": "Dot" },
            "face": { "size": 256, "distance": "Cosine" }
        });

        let error = validate_collection_vectors("images", 512, 256, &vectors).unwrap_err();

        assert!(error.contains("collection `images` is incompatible"));
        assert!(error.contains("expected named vectors `visual` size 512 and `face` size 256"));
        assert!(error.contains("found visual vector size 384 with Dot distance"));
        assert!(error.contains("set QDRANT_COLLECTION to a new empty collection name"));
    }

    #[test]
    fn legacy_collection_schema_is_rejected() {
        let vectors = json!({ "size": 512, "distance": "Cosine" });

        let error = validate_collection_vectors("images", 512, 256, &vectors).unwrap_err();

        assert!(error.contains("uses legacy vector schema"));
    }

    #[test]
    fn required_payload_indexes_cover_filter_fields() {
        let indexes = required_payload_indexes();
        let fields = indexes
            .iter()
            .map(|index| (index.field_name, index.field_schema))
            .collect::<Vec<_>>();

        assert_eq!(
            fields,
            vec![
                ("point_kind", PayloadFieldSchema::Keyword),
                ("id", PayloadFieldSchema::Keyword),
                ("source_uri", PayloadFieldSchema::Keyword),
                ("source_item_uri", PayloadFieldSchema::Keyword),
                ("source_type", PayloadFieldSchema::Keyword),
                ("media_kind", PayloadFieldSchema::Keyword),
                ("photo_has_gps", PayloadFieldSchema::Bool),
                ("width", PayloadFieldSchema::Integer),
                ("height", PayloadFieldSchema::Integer),
                ("size_bytes", PayloadFieldSchema::Integer),
                ("modified_at", PayloadFieldSchema::Float),
                ("photo_capture_time_epoch", PayloadFieldSchema::Float),
            ]
        );
    }

    #[test]
    fn payload_index_schema_accepts_compatible_types() {
        let schema = json!({
            "media_kind": { "data_type": "keyword" },
            "photo_has_gps": { "data_type": "bool" },
            "width": { "data_type": "integer" },
            "modified_at": { "data_type": "float" }
        });

        assert!(payload_index_type_matches(
            payload_index_type(Some(&schema), "media_kind").unwrap(),
            PayloadFieldSchema::Keyword
        ));
        assert!(payload_index_type_matches(
            payload_index_type(Some(&schema), "photo_has_gps").unwrap(),
            PayloadFieldSchema::Bool
        ));
        assert!(payload_index_type_matches(
            payload_index_type(Some(&schema), "width").unwrap(),
            PayloadFieldSchema::Integer
        ));
        assert!(payload_index_type_matches(
            payload_index_type(Some(&schema), "modified_at").unwrap(),
            PayloadFieldSchema::Float
        ));
    }

    #[test]
    fn incompatible_payload_index_schema_reports_remediation() {
        let error = payload_index_schema_error(
            "images",
            "modified_at",
            PayloadFieldSchema::Float,
            "keyword",
        );

        assert!(error.contains("payload index `modified_at` is incompatible"));
        assert!(error.contains("expected float, found keyword"));
        assert!(error.contains("set QDRANT_COLLECTION to a new empty collection name"));
    }

    #[test]
    fn qdrant_retry_classifier_matches_transient_http_statuses() {
        for status in [
            StatusCode::REQUEST_TIMEOUT,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::GATEWAY_TIMEOUT,
        ] {
            assert!(is_retryable_status(status), "{status} should be retryable");
        }

        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            StatusCode::CONFLICT,
            StatusCode::UNPROCESSABLE_ENTITY,
        ] {
            assert!(
                !is_retryable_status(status),
                "{status} should not be retryable"
            );
        }
    }

    #[test]
    fn retry_backoff_is_exponential_and_capped() {
        assert_eq!(retry_delay_ms(100, 0), 100);
        assert_eq!(retry_delay_ms(100, 1), 200);
        assert_eq!(retry_delay_ms(100, 4), 1_000);
    }

    #[test]
    fn media_search_filter_serializes_exact_and_range_conditions() {
        let filter = media_search_filter(Some(MediaSearchFilter {
            source_type: Some("s3".to_string()),
            media_kind: Some("static_image".to_string()),
            has_gps: Some(true),
            min_width: Some(640),
            max_width: Some(1920),
            modified_from: Some(1_700_000_000.0),
            ..MediaSearchFilter::default()
        }))
        .unwrap();

        let value = serde_json::to_value(filter).unwrap();
        assert_eq!(
            value,
            json!({
                "must": [
                    { "key": "point_kind", "match": { "value": "media" } },
                    { "key": "source_type", "match": { "value": "s3" } },
                    { "key": "media_kind", "match": { "value": "static_image" } },
                    { "key": "photo_has_gps", "match": { "value": true } },
                    { "key": "width", "range": { "gte": 640.0, "lte": 1920.0 } },
                    { "key": "modified_at", "range": { "gte": 1_700_000_000.0 } }
                ]
            })
        );
    }
}
