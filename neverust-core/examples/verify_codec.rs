use neverust_core::{Manifest, BLOCK_CODEC, MANIFEST_CODEC};

fn main() {
    // Create test CID
    let tree_data = b"test";
    let hash = blake3::hash(tree_data);
    let hash_bytes = hash.as_bytes();

    let mut buf = [0u8; 10];
    let mut multihash = Vec::new();
    let encoded = unsigned_varint::encode::u64(0x1e, &mut buf);
    multihash.extend_from_slice(encoded);
    let encoded = unsigned_varint::encode::u64(32, &mut buf);
    multihash.extend_from_slice(encoded);
    multihash.extend_from_slice(hash_bytes);

    let mut cid_bytes = Vec::new();
    let encoded = unsigned_varint::encode::u64(1, &mut buf);
    cid_bytes.extend_from_slice(encoded);
    let encoded = unsigned_varint::encode::u64(BLOCK_CODEC, &mut buf);
    cid_bytes.extend_from_slice(encoded);
    cid_bytes.extend_from_slice(&multihash);

    let tree_cid = cid::Cid::try_from(cid_bytes).unwrap();

    let manifest = Manifest::new(tree_cid, 1024, 1024, None, None, None, None, None);
    let block = manifest.to_block().unwrap();

    println!("Manifest codec constant: 0x{:x}", MANIFEST_CODEC);
    println!("Block CID codec: 0x{:x}", block.cid.codec());
    println!("Match: {}", block.cid.codec() == MANIFEST_CODEC);
    println!("Block CID: {}", block.cid);
}
