impl QdrantImageStore {
    async fn send_qdrant(
        &self,
        operation: &'static str,
        path: &str,
        build_request: impl Fn(&str) -> RequestBuilder,
    ) -> Result<Response, QdrantHttpError> {
        let max_attempts = self.http_options.retry_attempts.saturating_add(1).max(1);
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            for base_url in &self.base_urls {
                let fallback_url = format!("{base_url}{path}");
                match build_request(base_url).send().await {
                    Ok(response) if response.status().is_success() => return Ok(response),
                    Ok(response) => {
                        let status = response.status();
                        let url = response.url().to_string();
                        let body = response.text().await.unwrap_or_default();
                        let error = QdrantHttpError::http(
                            operation,
                            &self.collection,
                            url,
                            attempt,
                            status,
                            body,
                        );
                        if !is_retryable_status(status) {
                            return Err(error);
                        }
                        last_error = Some(error);
                    }
                    Err(error) => {
                        let url = error.url().map(ToString::to_string).unwrap_or(fallback_url);
                        let error = QdrantHttpError::request(
                            operation,
                            &self.collection,
                            url,
                            attempt,
                            error.to_string(),
                        );
                        last_error = Some(error);
                    }
                }
            }

            if attempt < max_attempts {
                let delay_ms = retry_delay_ms(self.http_options.retry_backoff_ms, attempt - 1);
                if let Some(error) = &last_error {
                    tracing::warn!(
                        operation,
                        collection = %self.collection,
                        attempt,
                        max_attempts,
                        next_delay_ms = delay_ms,
                        error = %error,
                        "retrying transient Qdrant request failure"
                    );
                }
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }

        Err(last_error.unwrap_or_else(|| {
            QdrantHttpError::request(
                operation,
                &self.collection,
                path.to_string(),
                max_attempts,
                "no Qdrant URLs are configured".to_string(),
            )
        }))
    }
}
