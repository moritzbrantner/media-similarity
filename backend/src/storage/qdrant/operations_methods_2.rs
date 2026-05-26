impl QdrantImageStore {
    async fn search_named(
        &self,
        name: &'static str,
        vector: Vec<f32>,
        limit: u32,
        point_kind: Option<&'static str>,
    ) -> Result<Vec<ScoredPoint>, String> {
        self.search_named_with_filter(name, vector, limit, point_kind.map(kind_filter))
            .await
    }

    async fn search_named_with_filter(
        &self,
        name: &'static str,
        vector: Vec<f32>,
        limit: u32,
        filter: Option<Filter>,
    ) -> Result<Vec<ScoredPoint>, String> {
        let request = SearchRequest {
            vector: NamedSearchVector { name, vector },
            limit,
            with_payload: true,
            filter,
        };
        let operation = if name == VISUAL_VECTOR_NAME {
            "search_visual"
        } else {
            "search_faces"
        };
        let path = format!("/collections/{}/points/search", self.collection);
        let response = self
            .send_qdrant(operation, &path, |base_url| {
                self.client.post(format!("{base_url}{path}")).json(&request)
            })
            .await
            .map_err(|error| error.to_string())?;
        let response = self
            .parse_json::<SearchResponse>(operation, response)
            .await?;
        Ok(response
            .result
            .into_iter()
            .map(|point| ScoredPoint {
                payload: point.payload,
                score: point.score,
            })
            .collect())
    }

    #[allow(dead_code)]
    pub async fn scroll_payloads(&self) -> Result<Vec<Value>, String> {
        Ok(self
            .scroll_media_points()
            .await?
            .into_iter()
            .filter_map(|point| point.payload)
            .collect())
    }

    pub async fn scroll_media_points(&self) -> Result<Vec<StoredPoint>, String> {
        self.scroll_points(Some("media")).await
    }

    pub async fn scroll_face_points(&self) -> Result<Vec<StoredPoint>, String> {
        self.scroll_points(Some("face")).await
    }

    pub async fn scroll_media_points_by_filter(
        &self,
        id: Option<&str>,
        source_uri: Option<&str>,
        source_item_uri: Option<&str>,
    ) -> Result<Vec<StoredPoint>, String> {
        let mut conditions = vec![field_condition("point_kind", "media")];
        if let Some(id) = id {
            conditions.push(field_condition("id", id));
        }
        if let Some(source_uri) = source_uri {
            conditions.push(field_condition("source_uri", source_uri));
        }
        if let Some(source_item_uri) = source_item_uri {
            conditions.push(field_condition("source_item_uri", source_item_uri));
        }
        self.scroll_points_with_filter(Some(Filter { must: conditions }))
            .await
    }

    pub async fn scroll_face_points_by_media_ids(
        &self,
        media_ids: &[String],
    ) -> Result<Vec<StoredPoint>, String> {
        if media_ids.is_empty() {
            return Ok(Vec::new());
        }
        let media_ids = media_ids
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        Ok(self
            .scroll_face_points()
            .await?
            .into_iter()
            .filter(|point| {
                point
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("media_id"))
                    .and_then(Value::as_str)
                    .map(|media_id| media_ids.contains(media_id))
                    .unwrap_or(false)
            })
            .collect())
    }

    async fn scroll_points(
        &self,
        point_kind: Option<&'static str>,
    ) -> Result<Vec<StoredPoint>, String> {
        self.scroll_points_with_filter(point_kind.map(kind_filter))
            .await
    }

    async fn scroll_points_with_filter(
        &self,
        filter: Option<Filter>,
    ) -> Result<Vec<StoredPoint>, String> {
        let mut offset = None;
        let mut points = Vec::new();

        loop {
            let request = ScrollRequest {
                limit: 256,
                with_payload: true,
                with_vector: false,
                offset: offset.clone(),
                filter: filter.clone(),
            };
            let path = format!("/collections/{}/points/scroll", self.collection);
            let response = self
                .send_qdrant("scroll_points", &path, |base_url| {
                    self.client.post(format!("{base_url}{path}")).json(&request)
                })
                .await
                .map_err(|error| error.to_string())?;
            let response = self
                .parse_json::<ScrollResponse>("scroll_points", response)
                .await?;

            points.extend(response.result.points.into_iter().map(|point| StoredPoint {
                id: point.id,
                payload: point.payload,
            }));

            match response.result.next_page_offset {
                Some(next_offset) => offset = Some(next_offset),
                None => break,
            }
        }

        Ok(points)
    }

    #[allow(dead_code)]
    pub async fn count(&self) -> Result<u64, String> {
        let request = serde_json::json!({ "exact": true });
        let path = format!("/collections/{}/points/count", self.collection);
        let response = self
            .send_qdrant("count_points", &path, |base_url| {
                self.client.post(format!("{base_url}{path}")).json(&request)
            })
            .await
            .map_err(|error| error.to_string())?;
        let response = self
            .parse_json::<CountResponse>("count_points", response)
            .await?;
        Ok(response.result.count)
    }

    async fn parse_json<T: DeserializeOwned>(
        &self,
        operation: &'static str,
        response: Response,
    ) -> Result<T, String> {
        let url = response.url().to_string();
        response.json::<T>().await.map_err(|error| {
            QdrantJsonError {
                operation,
                collection: self.collection.clone(),
                url,
                detail: error.to_string(),
            }
            .to_string()
        })
    }
}
