pub struct EmbeddingClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl EmbeddingClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url,
            model,
            client: reqwest::Client::new(),
        }
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let body = serde_json::json!({
            "input": text,
            "model": self.model,
            "encoding_format": "float",
        });

        let resp = self
            .client
            .post(format!("{}/v1/embeddings", self.base_url))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Embedding request failed: {}", e))?;

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Embedding JSON parse: {}", e))?;

        let embedding = json["data"][0]["embedding"]
            .as_array()
            .ok_or("No embedding in response")?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();

        Ok(embedding)
    }

    pub async fn embed_batch(
        &self,
        texts: &[String],
        concurrency: usize,
    ) -> Result<Vec<Vec<f32>>, String> {
        use std::sync::Arc;

        let sem = Arc::new(tokio::sync::Semaphore::new(concurrency));
        let mut handles = Vec::with_capacity(texts.len());

        for text in texts {
            let text = text.clone();
            let sem = sem.clone();
            let client = self.client.clone();
            let base_url = self.base_url.clone();
            let model = self.model.clone();
            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await;
                let body = serde_json::json!({
                    "input": &text,
                    "model": model,
                    "encoding_format": "float",
                });
                let resp = client
                    .post(format!("{}/v1/embeddings", base_url))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("Embedding request failed: {}", e))?;
                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Embedding JSON parse: {}", e))?;
                let embedding = json["data"][0]["embedding"]
                    .as_array()
                    .ok_or("No embedding in response")?
                    .iter()
                    .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                    .collect();
                Ok::<Vec<f32>, String>(embedding)
            });
            handles.push(handle);
        }

        let mut embeddings = Vec::with_capacity(handles.len());
        for h in handles {
            embeddings.push(h.await.map_err(|e| format!("join: {}", e))??);
        }
        Ok(embeddings)
    }
}
