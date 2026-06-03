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
        let ledger = IndexingLedger::load(&self.settings.indexing_ledger_file);
        let ledger_sources = ledger.active_run.as_ref().map(|run| &run.sources);

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
                        let ledger_source = ledger_sources
                            .and_then(|sources| sources.get(&source_image.item_uri))
                            .filter(|ledger_source| {
                                ledger_source.matches_source(&source_image, &indexing_profile)
                            });
                        let is_current = source_current_for_plan(
                            ledger_source,
                            &indexed_records,
                            &source_image,
                            &indexing_profile,
                        );
                        if is_current {
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

    async fn index_one(
        &self,
        source_image: &SourceImage,
        recorder: &mut IndexRunRecorder,
    ) -> Result<IndexOneOutcome, String> {
        recorder.check_cancelled()?;
        let kind = source_file_kind(source_image);
        if source_image.is_video() {
            return self.index_video(source_image, recorder).await;
        }
        if source_image.is_audio() {
            return self.index_audio(source_image, recorder).await;
        }
        if source_image.is_pdf() {
            return self.index_pdf(source_image, recorder).await;
        }

        let (settings, workflow) = self.workflow_settings(kind)?;
        recorder.current_part("static_image", 0, 1);
        recorder.check_cancelled()?;
        let photo_metadata = if workflow.processor_enabled("photo.extract_metadata")
            && !source_image.is_video()
            && !source_image.is_audio()
            && !source_image.is_pdf()
        {
            source_image
                    .with_local_media_path(&settings, |path| match extract_photo_metadata(path) {
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
        recorder.check_cancelled()?;
        let media = source_image.load_media(&settings).await?;
        recorder.check_cancelled()?;
        let media_id = image_id_for_uri(&source_image.id_base);
        let face_analysis = analyze_faces_for_media(
            &settings,
            self.store.as_ref(),
            &media,
            &media_id,
            Some(source_image.source_uri.clone()),
            Some(source_image.item_uri.clone()),
        )
        .await;
        recorder.check_cancelled()?;
        let payload = self.build_payload(
            source_image,
            &media,
            &settings,
            PayloadBuildOptions::new(&face_analysis)
                .with_photo_metadata(photo_metadata)
                .with_animated_thumbnail(workflow.processor_enabled("thumbnail.ensure_animated")),
        )?;
        let vector = self
            .embedder
            .embed_media(&media.sampled_frames, settings.gif_motion_weight)?;
        recorder.check_cancelled()?;
        self.store.upsert_media(&payload, vector).await?;
        recorder.committed_point(&payload.id);
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

    async fn index_video(
        &self,
        source_image: &SourceImage,
        recorder: &mut IndexRunRecorder,
    ) -> Result<IndexOneOutcome, String> {
        let (settings, _workflow) = self.workflow_settings(MediaFileKind::Video)?;
        recorder.check_cancelled()?;
        let scenes = source_image
            .with_local_media_path(&settings, |path| {
                decode_source_video_scenes(path, &source_image.id_base, &settings)
            })
            .await?;
        let mut outcome = IndexOneOutcome::default();
        let total = scenes.len();
        for (index, scene) in scenes.iter().enumerate() {
            recorder.current_part("video_scene", index, total);
            recorder.check_cancelled()?;
            let id_base = format!("{}#scene={}", source_image.id_base, scene.scene_index + 1);
            let media_id = image_id_for_uri(&id_base);
            let face_analysis = analyze_faces_for_media(
                &settings,
                self.store.as_ref(),
                &scene.media,
                &media_id,
                Some(source_image.source_uri.clone()),
                Some(source_image.item_uri.clone()),
            )
            .await;
            recorder.check_cancelled()?;
            let payload = self.build_payload(
                source_image,
                &scene.media,
                &settings,
                PayloadBuildOptions::new(&face_analysis).with_video_scene(scene),
            )?;
            let vector = self
                .embedder
                .embed_media(&scene.media.sampled_frames, settings.gif_motion_weight)?;
            recorder.check_cancelled()?;
            let point_id = payload.id.clone();
            self.store.upsert_media(&payload, vector).await?;
            recorder.committed_point(&point_id);
            outcome.insert(point_id);
        }
        Ok(outcome)
    }

    async fn index_audio(
        &self,
        source_image: &SourceImage,
        recorder: &mut IndexRunRecorder,
    ) -> Result<IndexOneOutcome, String> {
        let (settings, _workflow) = self.workflow_settings(MediaFileKind::Audio)?;
        recorder.check_cancelled()?;
        let segments = source_image
            .with_local_media_path(&settings, |path| {
                decode_source_audio_segments(path, &source_image.id_base, &settings)
            })
            .await?;
        let mut outcome = IndexOneOutcome::default();
        let total = segments.len();
        for (index, segment) in segments.iter().enumerate() {
            recorder.current_part("audio_segment", index, total);
            recorder.check_cancelled()?;
            let face_analysis = FaceAnalysis::default();
            let payload = self.build_payload(
                source_image,
                &segment.media,
                &settings,
                PayloadBuildOptions::new(&face_analysis).with_audio_segment(segment),
            )?;
            let vector = self
                .embedder
                .embed_media(&segment.media.sampled_frames, settings.gif_motion_weight)?;
            recorder.check_cancelled()?;
            let point_id = payload.id.clone();
            self.store.upsert_media(&payload, vector).await?;
            recorder.committed_point(&point_id);
            outcome.insert(point_id);
        }
        Ok(outcome)
    }
}

fn source_current_for_plan(
    ledger_source: Option<&IndexLedgerSource>,
    indexed_records: &[IndexedSourceRecord],
    source_image: &SourceImage,
    indexing_profile: &str,
) -> bool {
    ledger_source
        .map(|ledger_source| {
            ledger_source.status == IndexLedgerSourceStatus::Completed
                && committed_records_are_current(
                    indexed_records,
                    &ledger_source.committed_point_ids,
                    source_image,
                    indexing_profile,
                )
        })
        .unwrap_or_else(|| source_is_current(indexed_records, source_image, indexing_profile))
}

#[cfg(test)]
mod indexer_planning_tests {
    use super::{
        source_current_for_plan, IndexLedgerSource, IndexLedgerSourceStatus, IndexedSourceRecord,
    };
    use crate::workers::sources::SourceImage;

    #[test]
    fn incomplete_ledger_entry_forces_pending_even_with_current_record() {
        let source_image = SourceImage::test_local_image("/images/cat.jpg", 42, 100.0);
        let record = current_record("point");
        let ledger_source = ledger_source(IndexLedgerSourceStatus::Running, vec!["point"]);

        assert!(!source_current_for_plan(
            Some(&ledger_source),
            &[record],
            &source_image,
            "profile"
        ));
    }

    #[test]
    fn completed_ledger_entry_skips_when_all_committed_points_are_current() {
        let source_image = SourceImage::test_local_image("/images/cat.jpg", 42, 100.0);
        let records = vec![current_record("first"), current_record("second")];
        let ledger_source =
            ledger_source(IndexLedgerSourceStatus::Completed, vec!["first", "second"]);

        assert!(source_current_for_plan(
            Some(&ledger_source),
            &records,
            &source_image,
            "profile"
        ));
    }

    #[test]
    fn completed_ledger_entry_with_missing_point_forces_pending() {
        let source_image = SourceImage::test_local_image("/images/cat.jpg", 42, 100.0);
        let records = vec![current_record("first")];
        let ledger_source =
            ledger_source(IndexLedgerSourceStatus::Completed, vec!["first", "missing"]);

        assert!(!source_current_for_plan(
            Some(&ledger_source),
            &records,
            &source_image,
            "profile"
        ));
    }

    #[test]
    fn missing_ledger_entry_preserves_legacy_current_record_skip() {
        let source_image = SourceImage::test_local_image("/images/cat.jpg", 42, 100.0);
        let record = current_record("point");

        assert!(source_current_for_plan(
            None,
            &[record],
            &source_image,
            "profile"
        ));
    }

    fn current_record(point_id: &str) -> IndexedSourceRecord {
        IndexedSourceRecord {
            point_id: point_id.to_string(),
            size_bytes: 42,
            modified_at: 100.0,
            indexing_profile: Some("profile".to_string()),
            analysis_complete: true,
        }
    }

    fn ledger_source(status: IndexLedgerSourceStatus, point_ids: Vec<&str>) -> IndexLedgerSource {
        IndexLedgerSource {
            source_uri: "/images".to_string(),
            source_item_uri: "/images/cat.jpg".to_string(),
            display_path: "/images/cat.jpg".to_string(),
            size_bytes: 42,
            modified_at: 100.0,
            indexing_profile: "profile".to_string(),
            status,
            committed_point_ids: point_ids.into_iter().map(str::to_string).collect(),
            current_part: None,
            error: None,
            updated_at: chrono::Utc::now(),
        }
    }
}
