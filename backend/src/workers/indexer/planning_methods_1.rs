impl ImageIndexer {
    pub fn new(
        settings: Settings,
        store: Arc<dyn MediaVectorStore>,
        embedder: Arc<dyn VisualEmbeddingBackend>,
    ) -> Self {
        Self {
            settings,
            store,
            embedder,
        }
    }

    pub async fn index_sources(&self) -> IndexResponse {
        self.index_missing_sources(None).await
    }

    pub async fn index_missing_sources(&self, context: Option<&JobContext>) -> IndexResponse {
        let plan = match self.plan_sources().await {
            Ok(plan) => plan,
            Err(error) => {
                return IndexResponse {
                    indexed: 0,
                    skipped: 0,
                    failed: 1,
                    pruned: 0,
                    collection: self.settings.qdrant_collection.clone(),
                    source_dir: self.settings.source_image_dir.to_string_lossy().to_string(),
                    sources: build_image_sources(&self.settings)
                        .iter()
                        .map(|source| source.uri())
                        .collect(),
                    errors: vec![format!("Could not prepare indexing plan: {error}")],
                };
            }
        };

        if let Some(context) = context {
            let _ = context.info(format!(
                "{} source file(s) already indexed; {} source file(s) need indexing",
                plan.already_indexed,
                plan.pending.len()
            ));
            let _ = context.metadata("already_indexed", plan.already_indexed.to_string());
            let _ = context.metadata("needs_indexing", plan.pending.len().to_string());
        }

        let mut indexed = 0;
        let mut pruned = 0;
        let skipped = plan.skipped + plan.already_indexed;
        let mut failed = 0;
        let mut errors = plan.errors;
        if !plan.prune_point_ids.is_empty() {
            let prune_count = plan.prune_point_ids.len();
            if let Some(context) = context {
                let _ = context.info(format!(
                    "pruning {prune_count} stale Qdrant record(s) before indexing"
                ));
            }
            match self.delete_generated_records(&plan.prune_point_ids).await {
                Ok(deleted) => {
                    pruned += deleted;
                    if let Some(context) = context {
                        let _ = context.metadata("pruned", pruned.to_string());
                    }
                }
                Err(error) => {
                    failed += 1;
                    errors.push(format!("Could not prune stale Qdrant records: {error}"));
                    if let Some(context) = context {
                        let _ =
                            context.warn(format!("could not prune stale Qdrant records: {error}"));
                    }
                }
            }
        }
        let total = plan.pending.len() as u64;
        if let Some(context) = context {
            if let Ok(progress) = index_progress(0, total, "indexing pending sources") {
                let _ = context.progress(progress);
            }
        }
        for (index, pending_source) in plan.pending.iter().enumerate() {
            let source_image = &pending_source.source_image;
            if let Some(context) = context {
                if let Err(error) = context.check_cancelled() {
                    errors.truncate(50);
                    let _ = context.metadata("indexed", indexed.to_string());
                    let _ = context.metadata("failed", failed.to_string());
                    let _ = context.metadata("skipped", skipped.to_string());
                    let _ = context.metadata("pruned", pruned.to_string());
                    let _ = context.warn(format!(
                        "indexing cancelled before {}",
                        source_image.display_path
                    ));
                    return IndexResponse {
                        indexed,
                        skipped,
                        failed,
                        pruned,
                        collection: self.settings.qdrant_collection.clone(),
                        source_dir: self.settings.source_image_dir.to_string_lossy().to_string(),
                        sources: plan.source_uris,
                        errors: {
                            errors.push(error.to_string());
                            errors
                        },
                    };
                }
                let _ = context.info(format!("indexing {}", source_image.display_path));
            }

            match self.index_one(source_image).await {
                Ok(outcome) => {
                    indexed += outcome.indexed;
                    let stale_point_ids = pending_source
                        .indexed_point_ids
                        .iter()
                        .filter(|id| !outcome.point_ids.contains(*id))
                        .cloned()
                        .collect::<Vec<_>>();
                    if !stale_point_ids.is_empty() {
                        match self.delete_generated_records(&stale_point_ids).await {
                            Ok(deleted) => {
                                pruned += deleted;
                                if let Some(context) = context {
                                    let _ = context.info(format!(
                                        "pruned {} stale record(s) for {}",
                                        stale_point_ids.len(),
                                        source_image.display_path
                                    ));
                                }
                            }
                            Err(error) => {
                                failed += 1;
                                errors.push(format!(
                                    "{}: could not prune stale Qdrant records: {error}",
                                    source_image.display_path
                                ));
                                if let Some(context) = context {
                                    let _ = context.warn(format!(
                                        "{}: could not prune stale Qdrant records: {error}",
                                        source_image.display_path
                                    ));
                                }
                            }
                        }
                    }
                }
                Err(error) => {
                    failed += 1;
                    errors.push(format!("{}: {error}", source_image.display_path));
                    if let Some(context) = context {
                        let _ = context.warn(format!("{}: {error}", source_image.display_path));
                    }
                }
            }

            if let Some(context) = context {
                let completed = index as u64 + 1;
                if let Ok(progress) = index_progress(
                    completed,
                    total,
                    format!("indexed {completed}/{total} pending source files"),
                ) {
                    let _ = context.progress(progress);
                }
            }
        }

        if let Some(context) = context {
            let _ = context.metadata("indexed", indexed.to_string());
            let _ = context.metadata("failed", failed.to_string());
            let _ = context.metadata("skipped", skipped.to_string());
            let _ = context.metadata("pruned", pruned.to_string());
            let _ = context.info(format!(
                "indexing complete: {indexed} media item(s), {skipped} skipped, {pruned} pruned, {failed} failed"
            ));
        }

        errors.truncate(50);
        IndexResponse {
            indexed,
            skipped,
            failed,
            pruned,
            collection: self.settings.qdrant_collection.clone(),
            source_dir: self.settings.source_image_dir.to_string_lossy().to_string(),
            sources: plan.source_uris,
            errors,
        }
    }
}
