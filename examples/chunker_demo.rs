use neverust_core::{Chunker, DEFAULT_BLOCK_SIZE};
use std::io;
use tokio::fs::File;

#[tokio::main]
async fn main() -> io::Result<()> {
    println!("Chunker Demo - Archivist-compatible data chunking");
    println!("================================================\n");

    // Example 1: Chunking a string
    println!("Example 1: Chunking 'hello world' with 5-byte chunks");
    let data = b"hello world";
    let mut chunker = Chunker::with_chunk_size(&data[..], 5);

    let mut chunk_num = 1;
    while let Some(chunk) = chunker.next_chunk().await? {
        println!(
            "  Chunk {}: {:?} ({} bytes)",
            chunk_num,
            String::from_utf8_lossy(&chunk),
            chunk.len()
        );
        chunk_num += 1;
    }
    println!();

    // Example 2: Using default block size (64KB)
    println!(
        "Example 2: Default Archivist block size = {} bytes ({} KB)",
        DEFAULT_BLOCK_SIZE,
        DEFAULT_BLOCK_SIZE / 1024
    );
    let data = vec![0u8; DEFAULT_BLOCK_SIZE * 2 + 1000]; // 2.x blocks
    let mut chunker = Chunker::new(&data[..]);

    let mut total_chunks = 0;
    while let Some(chunk) = chunker.next_chunk().await? {
        total_chunks += 1;
        println!("  Chunk {}: {} bytes", total_chunks, chunk.len());
    }
    println!("  Total chunks: {}\n", total_chunks);

    // Example 3: Chunking a file (if Cargo.toml exists)
    if let Ok(file) = File::open("Cargo.toml").await {
        println!("Example 3: Chunking Cargo.toml");
        let mut chunker = Chunker::with_chunk_size(file, 256);

        let mut total_chunks = 0;
        let mut total_bytes = 0;
        while let Some(chunk) = chunker.next_chunk().await? {
            total_chunks += 1;
            total_bytes += chunk.len();
        }
        println!("  Total chunks: {}", total_chunks);
        println!("  Total bytes: {}", total_bytes);
        println!("  Chunk size: 256 bytes\n");
    }

    println!("Demo complete!");
    Ok(())
}
