#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    image_similarity_service::app::run().await
}
