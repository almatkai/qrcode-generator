#[tokio::main(worker_threads = 2)]
async fn main() {
    let result = match tezbar_qrcode_generator::Adapter::production() {
        Ok(adapter) => tezbar_extension_sdk::serve(adapter).await,
        Err(e) => Err(e),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
