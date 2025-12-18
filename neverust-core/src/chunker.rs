use std::io;
use tokio::io::{AsyncRead, AsyncReadExt};

/// Default block size for Archivist compatibility: 64KB (64 * 1024 bytes)
pub const DEFAULT_BLOCK_SIZE: usize = 65536;

/// A chunker that reads data from an async reader and splits it into fixed-size chunks
pub struct Chunker<R> {
    reader: R,
    chunk_size: usize,
    eof_reached: bool,
}

impl<R: AsyncRead + Unpin> Chunker<R> {
    /// Create a new chunker with the default block size (64KB)
    pub fn new(reader: R) -> Self {
        Self::with_chunk_size(reader, DEFAULT_BLOCK_SIZE)
    }

    /// Create a new chunker with a custom chunk size
    pub fn with_chunk_size(reader: R, chunk_size: usize) -> Self {
        assert!(chunk_size > 0, "chunk_size must be greater than 0");
        Self {
            reader,
            chunk_size,
            eof_reached: false,
        }
    }

    /// Read the next chunk from the reader
    ///
    /// Returns:
    /// - `Ok(Some(Vec<u8>))` - Next chunk of data (may be smaller than chunk_size at EOF)
    /// - `Ok(None)` - EOF reached, no more data
    /// - `Err(io::Error)` - IO error occurred
    pub async fn next_chunk(&mut self) -> io::Result<Option<Vec<u8>>> {
        if self.eof_reached {
            return Ok(None);
        }

        let mut buffer = vec![0u8; self.chunk_size];
        let mut total_read = 0;

        // Read up to chunk_size bytes
        while total_read < self.chunk_size {
            match self.reader.read(&mut buffer[total_read..]).await? {
                0 => {
                    // EOF reached
                    self.eof_reached = true;
                    if total_read == 0 {
                        return Ok(None);
                    } else {
                        // Return partial chunk
                        buffer.truncate(total_read);
                        return Ok(Some(buffer));
                    }
                }
                n => {
                    total_read += n;
                }
            }
        }

        Ok(Some(buffer))
    }

}

impl<R> Chunker<R> {
    /// Get the configured chunk size
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Check if EOF has been reached
    pub fn is_eof(&self) -> bool {
        self.eof_reached
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chunking_hello_world_with_5_byte_chunks() {
        let data = b"hello world";
        let mut chunker = Chunker::with_chunk_size(&data[..], 5);

        // First chunk: "hello"
        let chunk1 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk1, Some(b"hello".to_vec()));

        // Second chunk: " worl"
        let chunk2 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk2, Some(b" worl".to_vec()));

        // Third chunk: "d" (partial chunk at EOF)
        let chunk3 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk3, Some(b"d".to_vec()));

        // Fourth call: None (EOF)
        let chunk4 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk4, None);

        // Fifth call: Still None (EOF reached)
        let chunk5 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk5, None);
    }

    #[tokio::test]
    async fn test_exact_chunk_size_boundaries() {
        let data = b"0123456789"; // 10 bytes
        let mut chunker = Chunker::with_chunk_size(&data[..], 5);

        // First chunk: "01234"
        let chunk1 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk1, Some(b"01234".to_vec()));

        // Second chunk: "56789"
        let chunk2 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk2, Some(b"56789".to_vec()));

        // Third call: None (EOF, no partial chunk)
        let chunk3 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk3, None);
    }

    #[tokio::test]
    async fn test_empty_input() {
        let data = b"";
        let mut chunker = Chunker::with_chunk_size(&data[..], 64);

        // First call: None (EOF immediately)
        let chunk1 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk1, None);

        // Second call: Still None
        let chunk2 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk2, None);
    }

    #[tokio::test]
    async fn test_single_chunk_data_smaller_than_chunk_size() {
        let data = b"small";
        let mut chunker = Chunker::with_chunk_size(&data[..], 1024);

        // First chunk: "small" (smaller than chunk size)
        let chunk1 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk1, Some(b"small".to_vec()));

        // Second call: None (EOF)
        let chunk2 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk2, None);
    }

    #[tokio::test]
    async fn test_large_data_1mb_with_64kb_chunks() {
        // Create 1MB of data (1024 * 1024 bytes)
        let data_size = 1024 * 1024;
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

        let mut chunker = Chunker::with_chunk_size(&data[..], DEFAULT_BLOCK_SIZE);

        let mut total_bytes = 0;
        let mut chunk_count = 0;

        // Read all chunks
        while let Some(chunk) = chunker.next_chunk().await.unwrap() {
            chunk_count += 1;
            total_bytes += chunk.len();

            // All chunks except possibly the last should be DEFAULT_BLOCK_SIZE
            if chunk_count < 16 {
                // 1MB / 64KB = 16 chunks exactly
                assert_eq!(chunk.len(), DEFAULT_BLOCK_SIZE);
            }

            // Verify chunk data integrity
            let offset = (chunk_count - 1) * DEFAULT_BLOCK_SIZE;
            for (i, &byte) in chunk.iter().enumerate() {
                let expected = ((offset + i) % 256) as u8;
                assert_eq!(
                    byte, expected,
                    "Data mismatch at chunk {} offset {}",
                    chunk_count, i
                );
            }
        }

        // Verify we got exactly 16 chunks (1MB / 64KB)
        assert_eq!(chunk_count, 16);
        assert_eq!(total_bytes, data_size);
    }

    #[tokio::test]
    async fn test_default_chunk_size() {
        let data = vec![0u8; DEFAULT_BLOCK_SIZE + 100];
        let mut chunker = Chunker::new(&data[..]);

        // First chunk should be DEFAULT_BLOCK_SIZE
        let chunk1 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk1.unwrap().len(), DEFAULT_BLOCK_SIZE);

        // Second chunk should be 100 bytes (remainder)
        let chunk2 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk2.unwrap().len(), 100);

        // EOF
        let chunk3 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk3, None);
    }

    #[tokio::test]
    async fn test_chunk_size_getter() {
        let data = b"test";
        let chunker = Chunker::with_chunk_size(&data[..], 1024);
        assert_eq!(chunker.chunk_size(), 1024);

        let chunker_default = Chunker::new(&data[..]);
        assert_eq!(chunker_default.chunk_size(), DEFAULT_BLOCK_SIZE);
    }

    #[tokio::test]
    async fn test_eof_flag() {
        let data = b"test";
        let mut chunker = Chunker::with_chunk_size(&data[..], 10);

        assert!(!chunker.is_eof());

        let _chunk = chunker.next_chunk().await.unwrap();
        assert!(chunker.is_eof());

        let _none = chunker.next_chunk().await.unwrap();
        assert!(chunker.is_eof());
    }

    #[test]
    #[should_panic(expected = "chunk_size must be greater than 0")]
    fn test_zero_chunk_size_panics() {
        let data = b"test";
        let _chunker = Chunker::with_chunk_size(&data[..], 0);
    }

    #[tokio::test]
    async fn test_single_byte_chunks() {
        let data = b"abc";
        let mut chunker = Chunker::with_chunk_size(&data[..], 1);

        let chunk1 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk1, Some(b"a".to_vec()));

        let chunk2 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk2, Some(b"b".to_vec()));

        let chunk3 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk3, Some(b"c".to_vec()));

        let chunk4 = chunker.next_chunk().await.unwrap();
        assert_eq!(chunk4, None);
    }

    #[tokio::test]
    async fn test_archivist_default_block_size_value() {
        // Verify DEFAULT_BLOCK_SIZE matches Archivist's DefaultBlockSize (64KB)
        assert_eq!(DEFAULT_BLOCK_SIZE, 65536);
        assert_eq!(DEFAULT_BLOCK_SIZE, 64 * 1024);
    }
}
