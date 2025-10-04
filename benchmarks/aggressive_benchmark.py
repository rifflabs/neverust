#!/usr/bin/env python3
"""
Aggressive Benchmark - Massive Block Replication with Auto-Pruning

This benchmark demonstrates Neverust's ability to:
1. Generate blocks in-memory on each node
2. Replicate across the cluster rapidly
3. Automatically prune blocks once sufficient replication is achieved
4. Maintain high data availability with minimal per-node storage
"""

import asyncio
import aiohttp
import random
import hashlib
import time
from typing import List, Dict, Set
import json

# Configuration
NODES = [
    {'name': 'bootstrap', 'port': 9080},
    {'name': 'node1', 'port': 9081},
    {'name': 'node2', 'port': 9082},
    {'name': 'node3', 'port': 9083},
]

BLOCKS_PER_NODE = 16  # Each node generates up to 16 blocks
TARGET_REPLICATION = 3  # Blocks should be on at least 3 nodes
BLOCK_SIZE_MIN = 1024  # 1KB
BLOCK_SIZE_MAX = 102400  # 100KB
PRUNE_CHECK_INTERVAL = 2  # Check every 2 seconds if we can prune

# Track which blocks are where
block_locations: Dict[str, Set[str]] = {}  # CID -> set of node names
blocks_generated: Set[str] = set()
blocks_pruned: Set[str] = set()

async def generate_block(session: aiohttp.ClientSession, node: Dict) -> str:
    """Generate a random block on a node and return its CID"""
    size = random.randint(BLOCK_SIZE_MIN, BLOCK_SIZE_MAX)
    data = random.randbytes(size)

    url = f"http://localhost:{node['port']}/api/archivist/v1/data"

    try:
        async with session.post(url, data=data, headers={'Content-Type': 'application/octet-stream'}) as resp:
            if resp.status == 200:
                cid = await resp.text()
                cid = cid.strip('"')
                print(f"✅ [{node['name']}] Generated block {cid[:16]}... ({size} bytes)")

                # Track location
                if cid not in block_locations:
                    block_locations[cid] = set()
                block_locations[cid].add(node['name'])
                blocks_generated.add(cid)

                return cid
            else:
                print(f"❌ [{node['name']}] Failed to generate block: {resp.status}")
                return None
    except Exception as e:
        print(f"❌ [{node['name']}] Error generating block: {e}")
        return None

async def request_block_from_network(session: aiohttp.ClientSession, node: Dict, cid: str) -> bool:
    """Request a block from the network via a specific node"""
    url = f"http://localhost:{node['port']}/api/archivist/v1/data/{cid}/network/stream"

    try:
        async with session.get(url) as resp:
            if resp.status == 200:
                data = await resp.read()
                print(f"📥 [{node['name']}] Received {cid[:16]}... ({len(data)} bytes)")

                # Track that this node now has the block
                if cid not in block_locations:
                    block_locations[cid] = set()
                block_locations[cid].add(node['name'])

                return True
            else:
                return False
    except Exception as e:
        return False

async def prune_block(session: aiohttp.ClientSession, node: Dict, cid: str) -> bool:
    """Delete a block from a node"""
    url = f"http://localhost:{node['port']}/api/archivist/v1/data/{cid}"

    try:
        async with session.delete(url) as resp:
            if resp.status == 200:
                print(f"🗑️  [{node['name']}] Pruned {cid[:16]}... (replication sufficient)")

                # Update tracking
                if cid in block_locations and node['name'] in block_locations[cid]:
                    block_locations[cid].remove(node['name'])
                blocks_pruned.add(cid)

                return True
            else:
                return False
    except Exception as e:
        return False

async def check_block_exists(session: aiohttp.ClientSession, node: Dict, cid: str) -> bool:
    """Check if a node has a specific block"""
    url = f"http://localhost:{node['port']}/api/archivist/v1/data/{cid}"

    try:
        async with session.head(url) as resp:
            return resp.status == 200
    except:
        return False

async def get_node_stats(session: aiohttp.ClientSession, node: Dict) -> Dict:
    """Get current stats from a node"""
    url = f"http://localhost:{node['port']}/api/archivist/v1/stats"

    try:
        async with session.get(url) as resp:
            if resp.status == 200:
                return await resp.json()
            return {}
    except:
        return {}

async def replication_worker(session: aiohttp.ClientSession):
    """Continuously replicate blocks to achieve target replication"""
    while True:
        for cid in list(blocks_generated):
            if cid in blocks_pruned:
                continue

            current_replicas = len(block_locations.get(cid, set()))

            if current_replicas < TARGET_REPLICATION:
                # Find nodes that don't have this block
                nodes_without = [n for n in NODES if n['name'] not in block_locations.get(cid, set())]

                if nodes_without:
                    target_node = random.choice(nodes_without)
                    await request_block_from_network(session, target_node, cid)

        await asyncio.sleep(0.5)  # Check every 500ms

async def pruning_worker(session: aiohttp.ClientSession):
    """Continuously prune blocks that have achieved sufficient replication"""
    while True:
        for cid in list(blocks_generated):
            if cid in blocks_pruned:
                continue

            locations = block_locations.get(cid, set())

            # If we have more than target replication, prune from random nodes
            if len(locations) > TARGET_REPLICATION:
                # Keep TARGET_REPLICATION, prune the rest
                nodes_to_prune = list(locations)[TARGET_REPLICATION:]

                for node_name in nodes_to_prune:
                    node = next((n for n in NODES if n['name'] == node_name), None)
                    if node:
                        await prune_block(session, node, cid)

        await asyncio.sleep(PRUNE_CHECK_INTERVAL)

async def stats_reporter(session: aiohttp.ClientSession):
    """Report aggregate statistics"""
    while True:
        print("\n" + "="*80)
        print(f"📊 BENCHMARK STATISTICS (t={time.time():.1f}s)")
        print("="*80)

        total_blocks_generated = len(blocks_generated)
        total_blocks_active = len([cid for cid in blocks_generated if cid not in blocks_pruned])
        total_replicas = sum(len(locs) for locs in block_locations.values())

        print(f"Blocks Generated: {total_blocks_generated}")
        print(f"Blocks Active: {total_blocks_active}")
        print(f"Total Block Instances: {total_replicas}")
        print(f"Avg Replication Factor: {total_replicas / max(total_blocks_active, 1):.2f}")

        # Per-node stats
        for node in NODES:
            stats = await get_node_stats(session, node)
            blocks_stored = len([cid for cid, locs in block_locations.items() if node['name'] in locs])
            print(f"  [{node['name']}] Blocks: {blocks_stored}, Total Size: {stats.get('total_size', 0)} bytes")

        print("="*80 + "\n")

        await asyncio.sleep(5)  # Report every 5 seconds

async def generation_phase(session: aiohttp.ClientSession):
    """Phase 1: Generate blocks on each node"""
    print("\n🚀 PHASE 1: BLOCK GENERATION")
    print("="*80)

    tasks = []
    for node in NODES:
        for i in range(BLOCKS_PER_NODE):
            tasks.append(generate_block(session, node))

    await asyncio.gather(*tasks)
    print(f"\n✅ Generated {len(blocks_generated)} total blocks\n")

async def main():
    print("🔥 AGGRESSIVE BENCHMARK - MASSIVE REPLICATION")
    print("="*80)
    print(f"Nodes: {len(NODES)}")
    print(f"Blocks per node: {BLOCKS_PER_NODE}")
    print(f"Target replication: {TARGET_REPLICATION}")
    print(f"Block size range: {BLOCK_SIZE_MIN}-{BLOCK_SIZE_MAX} bytes")
    print("="*80 + "\n")

    async with aiohttp.ClientSession() as session:
        # Phase 1: Generate blocks
        await generation_phase(session)

        # Phase 2: Start background workers
        print("🚀 PHASE 2: REPLICATION & PRUNING")
        print("="*80 + "\n")

        await asyncio.gather(
            replication_worker(session),
            pruning_worker(session),
            stats_reporter(session)
        )

if __name__ == '__main__':
    asyncio.run(main())
