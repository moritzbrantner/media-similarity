impl ImageIndexer {
    async fn plan_sources(&self) -> Result<SourceIndexPlan, String> {
        let sources = build_image_sources(&self.settings);
        let source_uris = sources
            .iter()
            .map(|source| source.uri())
            .collect::<Vec<_>>();

        self.store.ensure_collection().await?;
        let indexing_profile = indexing_profile(&self.settings);
        let indexed_sources = self.indexed_source_records().await?;

        let mut pending = Vec::new();
        let mut already_indexed = 0;
        let mut skipped = 0;
        let mut errors = Vec::new();
        let mut scanned_source_items = BTreeSet::new();
        let mut prune_point_ids = Vec::new();
        for source in &sources {
            match source.iter_images().await {
                Ok(images) => {
                    for source_image in images {
                        scanned_source_items.insert(source_image.item_uri.clone());
                        let indexed_records = indexed_sources
                            .get(&source_image.item_uri)
                            .cloned()
                            .unwrap_or_default();
                        if source_is_current(&indexed_records, &source_image, &indexing_profile) {
                            already_indexed += 1;
                            prune_point_ids.extend(
                                indexed_records
                                    .iter()
                                    .filter(|record| {
                                        !record_is_current(record, &source_image, &indexing_profile)
                                    })
                                    .map(|record| record.point_id.clone()),
                            );
                        } else {
                            pending.push(PendingSource {
                                source_image,
                                indexed_point_ids: indexed_records
                                    .iter()
                                    .map(|record| record.point_id.clone())
                                    .collect(),
                            });
                        }
                    }
                }
                Err(SourceUnavailable(error)) => {
                    skipped += 1;
                    errors.push(error);
                }
            }
        }

        prune_point_ids.extend(
            indexed_sources
                .iter()
                .filter(|(source_item_uri, _)| !scanned_source_items.contains(*source_item_uri))
                .flat_map(|(_, records)| records.iter().map(|record| record.point_id.clone())),
        );
        prune_point_ids.sort();
        prune_point_ids.dedup();

        errors.truncate(50);
        Ok(SourceIndexPlan {
            source_uris,
            pending,
            already_indexed,
            skipped,
            prune_point_ids,
            errors,
        })
    }

    async fn indexed_source_records(
        &self,
    ) -> Result<BTreeMap<String, Vec<IndexedSourceRecord>>, String> {
        let mut records = BTreeMap::<String, Vec<IndexedSourceRecord>>::new();
        for point in self.store.scroll_media_points().await? {
            let Some(payload) = point.payload else {
                continue;
            };
            let Ok(payload) = serde_json::from_value::<ImagePayload>(payload) else {
                continue;
            };
            let Some(source_item_uri) = payload
                .source_item_uri
                .clone()
                .or_else(|| legacy_source_item_uri(&payload))
            else {
                continue;
            };
            records
                .entry(source_item_uri)
                .or_default()
                .push(IndexedSourceRecord {
                    point_id: point.id,
                    size_bytes: payload.size_bytes,
                    modified_at: payload.modified_at,
                    indexing_profile: payload.indexing_profile.clone(),
                    analysis_complete: payload_analysis_complete(&payload, &self.settings),
                });
        }
        Ok(records)
    }

    async fn index_one(&self, source_image: &SourceImage) -> Result<IndexOneOutcome, String> {
        if source_image.is_video() {
            return self.index_video(source_image).await;
        }
        if source_image.is_audio() {
            return self.index_audio(source_image).await;
        }
        if source_image.is_pdf() {
            return self.index_pdf(source_image).await;
        }

        let photo_metadata = if !source_image.is_video()
            && !source_image.is_audio()
            && !source_image.is_pdf()
        {
            source_image
                    .with_local_media_path(&self.settings, |path| match extract_photo_metadata(path) {
                        Ok(metadata) => Ok(metadata),
                        Err(error) => {
                            tracing::warn!(%error, path = %path.display(), "photo metadata extraction failed");
                            Ok(None)
                        }
                    })
                    .await?
        } else {
            None
        };
        let media = source_image.load_media(&self.settings).await?;
        let media_id = image_id_for_uri(&source_image.id_base);
        let face_analysis = analyze_faces_for_media(
            &self.settings,
            self.store.as_ref(),
            &media,
            &media_id,
            Some(source_image.source_uri.clone()),
            Some(source_image.item_uri.clone()),
        )
        .await;
        let payload = self.build_payload(
            source_image,
            &media,
            PayloadBuildOptions::new(&face_analysis).with_photo_metadata(photo_metadata),
        )?;
        let vector = self
            .embedder
            .embed_media(&media.sampled_frames, self.settings.gif_motion_weight)?;
        self.store.upsert_media(&payload, vector).await?;
        Ok(IndexOneOutcome::single(payload.id))
    }

    async fn delete_generated_records(&self, point_ids: &[String]) -> Result<usize, String> {
        let mut deleted = 0;
        let mut errors = Vec::new();
        for point_id in point_ids {
            let response =
                delete_indexed_media(&self.settings, self.store.as_ref(), point_id).await;
            deleted += response.deleted_points;
            errors.extend(response.errors);
        }
        if errors.is_empty() {
            Ok(deleted)
        } else {
            Err(errors.join("; "))
        }
    }

    async fn index_video(&self, source_image: &SourceImage) -> Result<IndexOneOutcome, String> {
        let scenes = source_image
            .with_local_media_path(&self.settings, |path| {
                decode_source_video_scenes(path, &source_image.id_base, &self.settings)
            })
            .await?;
        let mut outcome = IndexOneOutcome::default();
        for scene in &scenes {
            let id_base = format!("{}#scene={}", source_image.id_base, scene.scene_index + 1);
            let media_id = image_id_for_uri(&id_base);
            let face_analysis = analyze_faces_for_media(
                &self.settings,
                self.store.as_ref(),
                &scene.media,
                &media_id,
                Some(source_image.source_uri.clone()),
                Some(source_image.item_uri.clone()),
            )
            .await;
            let payload = self.build_payload(
                source_image,
                &scene.media,
                PayloadBuildOptions::new(&face_analysis).with_video_scene(scene),
            )?;
            let vector = self
                .embedder
                .embed_media(&scene.media.sampled_frames, self.settings.gif_motion_weight)?;
            let point_id = payload.id.clone();
            self.store.upsert_media(&payload, vector).await?;
            outcome.insert(point_id);
        }
        Ok(outcome)
    }

    async fn index_audio(&self, source_image: &SourceImage) -> Result<IndexOneOutcome, String> {
        let segments = source_image
            .with_local_media_path(&self.settings, |path| {
                decode_source_audio_segments(path, &source_image.id_base, &self.settings)
            })
            .await?;
        let mut outcome = IndexOneOutcome::default();
        for segment in &segments {
            let face_analysis = FaceAnalysis::default();
            let payload = self.build_payload(
                source_image,
                &segment.media,
                PayloadBuildOptions::new(&face_analysis).with_audio_segment(segment),
            )?;
            let vector = self.embedder.embed_media(
                &segment.media.sampled_frames,
                self.settings.gif_motion_weight,
            )?;
            let point_id = payload.id.clone();
            self.store.upsert_media(&payload, vector).await?;
            outcome.insert(point_id);
        }
        Ok(outcome)
    }
}
