impl QdrantImageStore {
    pub fn new(
        url: impl Into<String>,
        collection: impl Into<String>,
        visual_vector_size: usize,
        face_vector_size: usize,
    ) -> Self {
        Self::new_with_options(
            url,
            collection,
            visual_vector_size,
            face_vector_size,
            QdrantHttpOptions::default(),
        )
    }

    pub fn new_with_options(
        url: impl Into<String>,
        collection: impl Into<String>,
        visual_vector_size: usize,
        face_vector_size: usize,
        http_options: QdrantHttpOptions,
    ) -> Self {
        Self {
            client: qdrant_http_client(&http_options),
            base_urls: qdrant_base_urls(&url.into()),
            collection: collection.into(),
            visual_vector_size,
            face_vector_size,
            http_options,
        }
    }

    pub async fn ensure_collection(&self) -> Result<(), String> {
        let path = "/collections".to_string();
        let response = self
            .send_qdrant("list_collections", &path, |base_url| {
                self.client.get(format!("{base_url}{path}"))
            })
            .await
            .map_err(|error| error.to_string())?;
        let response = self
            .parse_json::<CollectionsResponse>("list_collections", response)
            .await?;

        if response
            .result
            .collections
            .iter()
            .any(|collection| collection.name == self.collection)
        {
            self.validate_collection_schema_and_indexes().await?;
            return Ok(());
        }

        self.create_collection().await?;
        self.validate_collection_schema_and_indexes().await?;
        Ok(())
    }

    async fn create_collection(&self) -> Result<(), String> {
        let request = CreateCollectionRequest {
            vectors: NamedVectors {
                visual: VectorParams {
                    size: self.visual_vector_size,
                    distance: EXPECTED_DISTANCE,
                },
                face: VectorParams {
                    size: self.face_vector_size,
                    distance: EXPECTED_DISTANCE,
                },
            },
        };
        let path = format!(
            "/collections/{}?timeout={}",
            self.collection,
            self.operation_timeout_seconds()
        );
        match self
            .send_qdrant("create_collection", &path, |base_url| {
                self.client.put(format!("{base_url}{path}")).json(&request)
            })
            .await
        {
            Ok(_) => Ok(()),
            Err(error) if error.status == Some(StatusCode::CONFLICT) => Ok(()),
            Err(error) => match self.fetch_collection_info().await {
                Ok(response) => {
                    validate_collection_vectors(
                        &self.collection,
                        self.visual_vector_size,
                        self.face_vector_size,
                        &response.result.config.params.vectors,
                    )?;
                    Ok(())
                }
                Err(_) => Err(error.to_string()),
            },
        }
    }

    async fn validate_collection_schema_and_indexes(&self) -> Result<(), String> {
        let response = self.fetch_collection_info().await?;

        validate_collection_vectors(
            &self.collection,
            self.visual_vector_size,
            self.face_vector_size,
            &response.result.config.params.vectors,
        )?;
        self.ensure_payload_indexes(response.result.payload_schema.as_ref())
            .await?;
        Ok(())
    }

    async fn fetch_collection_info(&self) -> Result<CollectionInfoResponse, String> {
        let path = format!("/collections/{}", self.collection);
        let response = self
            .send_qdrant("get_collection", &path, |base_url| {
                self.client.get(format!("{base_url}{path}"))
            })
            .await
            .map_err(|error| error.to_string())?;
        self.parse_json::<CollectionInfoResponse>("get_collection", response)
            .await
    }

    async fn ensure_payload_indexes(&self, payload_schema: Option<&Value>) -> Result<(), String> {
        for spec in required_payload_indexes() {
            match payload_index_type(payload_schema, spec.field_name) {
                Some(actual) if payload_index_type_matches(actual, spec.field_schema) => {}
                Some(actual) => {
                    return Err(payload_index_schema_error(
                        &self.collection,
                        spec.field_name,
                        spec.field_schema,
                        actual,
                    ));
                }
                None => self.create_payload_index(*spec).await?,
            }
        }
        Ok(())
    }

    async fn create_payload_index(&self, spec: PayloadIndexSpec) -> Result<(), String> {
        let request = CreatePayloadIndexRequest {
            field_name: spec.field_name,
            field_schema: spec.field_schema,
        };
        let path = format!(
            "/collections/{}/index?wait=true&timeout={}",
            self.collection,
            self.operation_timeout_seconds()
        );
        match self
            .send_qdrant("create_payload_index", &path, |base_url| {
                self.client.put(format!("{base_url}{path}")).json(&request)
            })
            .await
        {
            Ok(_) => Ok(()),
            Err(error) => match self.fetch_collection_info().await {
                Ok(response) => match payload_index_type(
                    response.result.payload_schema.as_ref(),
                    spec.field_name,
                ) {
                    Some(actual) if payload_index_type_matches(actual, spec.field_schema) => Ok(()),
                    Some(actual) => Err(payload_index_schema_error(
                        &self.collection,
                        spec.field_name,
                        spec.field_schema,
                        actual,
                    )),
                    None => Err(error.to_string()),
                },
                Err(_) => Err(error.to_string()),
            },
        }
    }

    fn operation_timeout_seconds(&self) -> u64 {
        self.http_options.request_timeout_ms.div_ceil(1_000).max(1)
    }

    pub async fn upsert_media(
        &self,
        payload: &ImagePayload,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        let mut payload_value = serde_json::to_value(payload).map_err(|error| error.to_string())?;
        set_payload_kind(&mut payload_value, "media");
        add_media_filter_payload_fields(&mut payload_value, payload);
        let request = UpsertRequest {
            points: vec![PointStruct {
                id: payload.id.clone(),
                vector: NamedPointVectors::visual(vector),
                payload: payload_value,
            }],
        };
        let path = format!("/collections/{}/points?wait=true", self.collection);
        self.send_qdrant("upsert_media", &path, |base_url| {
            self.client.put(format!("{base_url}{path}")).json(&request)
        })
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn upsert_face(
        &self,
        payload: &FacePointPayload,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        let mut payload_value = serde_json::to_value(payload).map_err(|error| error.to_string())?;
        set_payload_kind(&mut payload_value, "face");
        let request = UpsertRequest {
            points: vec![PointStruct {
                id: payload.face_id.clone(),
                vector: NamedPointVectors::face(vector),
                payload: payload_value,
            }],
        };
        let path = format!("/collections/{}/points?wait=true", self.collection);
        self.send_qdrant("upsert_face", &path, |base_url| {
            self.client.put(format!("{base_url}{path}")).json(&request)
        })
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn set_media_payload(&self, payload: &ImagePayload) -> Result<(), String> {
        let mut payload_value = serde_json::to_value(payload).map_err(|error| error.to_string())?;
        set_payload_kind(&mut payload_value, "media");
        add_media_filter_payload_fields(&mut payload_value, payload);
        let request = SetPayloadRequest {
            payload: payload_value,
            points: vec![payload.id.clone()],
        };
        let path = format!("/collections/{}/points/payload?wait=true", self.collection);
        self.send_qdrant("set_media_payload", &path, |base_url| {
            self.client.post(format!("{base_url}{path}")).json(&request)
        })
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn set_face_payload(&self, payload: &FacePointPayload) -> Result<(), String> {
        let mut payload_value = serde_json::to_value(payload).map_err(|error| error.to_string())?;
        set_payload_kind(&mut payload_value, "face");
        let request = SetPayloadRequest {
            payload: payload_value,
            points: vec![payload.face_id.clone()],
        };
        let path = format!("/collections/{}/points/payload?wait=true", self.collection);
        self.send_qdrant("set_face_payload", &path, |base_url| {
            self.client.post(format!("{base_url}{path}")).json(&request)
        })
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn delete_points(&self, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }

        let request = DeletePointsRequest {
            points: ids.to_vec(),
        };
        let path = format!("/collections/{}/points/delete?wait=true", self.collection);
        self.send_qdrant("delete_points", &path, |base_url| {
            self.client.post(format!("{base_url}{path}")).json(&request)
        })
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn delete_points_by_ids(&self, ids: &[String]) -> Result<(), String> {
        self.delete_points(ids).await
    }

    pub async fn search_visual(
        &self,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<ScoredPoint>, String> {
        self.search_named(VISUAL_VECTOR_NAME, vector, limit, Some("media"))
            .await
    }

    pub async fn search_visual_filtered(
        &self,
        vector: Vec<f32>,
        limit: u32,
        filter: Option<MediaSearchFilter>,
    ) -> Result<Vec<ScoredPoint>, String> {
        let filter = media_search_filter(filter);
        self.search_named_with_filter(VISUAL_VECTOR_NAME, vector, limit, filter)
            .await
    }

    pub async fn search_faces(
        &self,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<ScoredPoint>, String> {
        self.search_named(FACE_VECTOR_NAME, vector, limit, Some("face"))
            .await
    }
}
