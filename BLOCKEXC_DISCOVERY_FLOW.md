# BlockExc-Discovery Integration Flow Diagram

## Block Request Flow with Discovery Fallback

```
┌──────────────┐
│  Application │
│   (Client)   │
└──────┬───────┘
       │
       │ blockexc_client.request_block(cid)
       ▼
┌──────────────────────────────────────────────────────────────┐
│                      BlockExcClient                          │
│                                                              │
│  1. Check local blockstore first                             │
│     │                                                        │
│     ├─► Block found locally ──────────────────────┐         │
│     │                                              │         │
│     └─► Block not found locally                   │         │
│         │                                          │         │
│         ▼                                          │         │
│  2. Create BlockRequest                            │         │
│     │                                              │         │
│     ▼                                              │         │
│  3. Send request to swarm via channel              │         │
│     │                                              │         │
│     └─► Timeout (30s) ──► ERROR                   │         │
│                                                    │         │
└────────────────────────────────────────────────────┼─────────┘
                                                     │
                                                     ▼
                                              ┌─────────────┐
                                              │Return Block │
                                              └─────────────┘

┌──────────────────────────────────────────────────────────────┐
│                    BlockExcBehaviour                         │
│                                                              │
│  4. Receive BlockRequest from channel                        │
│     │                                                        │
│     ▼                                                        │
│  5. Store in pending_requests                                │
│     │                                                        │
│     ▼                                                        │
│  6. Broadcast WantBlock to all connected peers               │
│     │                                                        │
│     ├─► No peers connected ────┐                            │
│     │                           │                            │
│     ├─► Peers connected         │                            │
│     │   │                       │                            │
│     │   ▼                       │                            │
│     │   Queue RequestBlock events                            │
│     │   │                       │                            │
│     │   └─► Handlers send WantList to peers                 │
│     │                           │                            │
│     │                           │                            │
│     │   ┌───────────────────────┘                            │
│     │   │                                                    │
│     │   ▼                                                    │
│     │   7. Discovery fallback triggered                      │
│     │      │                                                 │
│     │      ▼                                                 │
│     │      if discovery.is_some() {                          │
│     │          queue_find_blocks(vec![cid])                  │
│     │      }                                                 │
│     │                                                        │
│     └─► Wait for block delivery...                          │
│                                                              │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│                      Discovery Queue                         │
│                    (process_discovery_queue)                 │
│                                                              │
│  8. Periodically called from poll()                          │
│     │                                                        │
│     ▼                                                        │
│  For each queued CID:                                        │
│     │                                                        │
│     ▼                                                        │
│  9. Check retry count                                        │
│     │                                                        │
│     ├─► retry_count >= 3 ──► Remove from queue              │
│     │                        (discovery failed)              │
│     │                        metrics.discovery_failure()     │
│     │                                                        │
│     └─► retry_count < 3                                      │
│         │                                                    │
│         ▼                                                    │
│  10. Call discovery.find(cid)                                │
│      metrics.discovery_query()                               │
│      │                                                       │
│      ▼                                                       │
└──────┼────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│                      Discovery.find(cid)                     │
│                                                              │
│  11. Check local provider cache                              │
│      │                                                       │
│      ├─► Providers in cache ──► Return cached PeerIds       │
│      │                                                       │
│      └─► Not in cache                                        │
│          │                                                   │
│          ▼                                                   │
│  12. Convert CID to NodeId (Keccak256)                       │
│      node_id = keccak256(cid.to_bytes())                     │
│      │                                                       │
│      ▼                                                       │
│  13. Find K closest DHT nodes                                │
│      closest_nodes = discv5.find_node(node_id)               │
│      │                                                       │
│      ▼                                                       │
│  14. Query top 3 nodes via TALK protocol                     │
│      │                                                       │
│      ├─► For each node:                                      │
│      │    │                                                  │
│      │    ▼                                                  │
│      │    Send GET_PROVIDERS request                         │
│      │    │                                                  │
│      │    ├─► Success: Receive ProviderRecords               │
│      │    │   │                                              │
│      │    │   ▼                                              │
│      │    │   Cache records locally                          │
│      │    │   │                                              │
│      │    │   └─► Add to all_providers                       │
│      │    │                                                  │
│      │    └─► Failure: Log warning, continue                 │
│      │                                                       │
│      ▼                                                       │
│  15. Convert ProviderRecords to PeerIds                      │
│      Deduplicate using HashSet                               │
│      │                                                       │
│      └─► Return Vec<PeerId>                                  │
│                                                              │
└──────────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│              Back to Discovery Queue Handler                 │
│                                                              │
│  16. Process discovery results                               │
│      │                                                       │
│      ├─► Providers found (providers.len() > 0)               │
│      │   │                                                   │
│      │   ▼                                                   │
│      │   metrics.discovery_success()                         │
│      │   │                                                   │
│      │   ▼                                                   │
│      │   For each provider:                                  │
│      │   │                                                   │
│      │   ├─► Provider is connected                           │
│      │   │   │                                               │
│      │   │   ▼                                               │
│      │   │   Queue RequestBlock event immediately            │
│      │   │   │                                               │
│      │   │   └─► Block will be requested via BlockExc        │
│      │   │                                                   │
│      │   └─► Provider not connected                          │
│      │       │                                               │
│      │       ▼                                               │
│      │       TODO: Dial provider first                       │
│      │       (log message for now)                           │
│      │   │                                                   │
│      │   ▼                                                   │
│      │   Remove CID from discovery_queue                     │
│      │   (discovery completed successfully)                  │
│      │                                                       │
│      └─► No providers found                                  │
│          │                                                   │
│          ▼                                                   │
│          Increment retry_count                               │
│          │                                                   │
│          └─► If retry_count >= 3:                            │
│              metrics.discovery_failure()                     │
│              Remove from queue                               │
│                                                              │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│                   Block Reception Handler                    │
│                  (on_connection_handler_event)               │
│                                                              │
│  17. BlockReceived event from peer                           │
│      │                                                       │
│      ▼                                                       │
│  18. Check if block was in discovery_queue                   │
│      │                                                       │
│      ├─► Yes (discovery-assisted retrieval)                  │
│      │   │                                                   │
│      │   ▼                                                   │
│      │   metrics.block_from_discovery()                      │
│      │   │                                                   │
│      │   ▼                                                   │
│      │   Remove from discovery_queue                         │
│      │   │                                                   │
│      │   └─► Continue normal block processing                │
│      │                                                       │
│      └─► No (standard BlockExc retrieval)                    │
│          │                                                   │
│          └─► Continue normal block processing                │
│              │                                               │
│              ▼                                               │
│  19. Store block in blockstore                               │
│      metrics.block_received(size)                            │
│      │                                                       │
│      ▼                                                       │
│  20. Complete pending request                                │
│      │                                                       │
│      ├─► Find matching BlockRequest                          │
│      │   │                                                   │
│      │   ▼                                                   │
│      │   Send block via oneshot channel                      │
│      │   │                                                   │
│      │   └─► BlockExcClient receives block                   │
│      │       │                                               │
│      │       ▼                                               │
│      │       Application receives block ✅                   │
│      │                                                       │
│      └─► No matching request (opportunistic block)           │
│          │                                                   │
│          └─► Just store in blockstore                        │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

## DHT Provider Advertisement Flow

```
┌──────────────┐
│ BlockStore   │
│   (Local)    │
└──────┬───────┘
       │
       │ New block stored
       ▼
┌──────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│                                                              │
│  1. Block successfully stored                                │
│     │                                                        │
│     ▼                                                        │
│  2. Call discovery.provide(&block.cid)                       │
│                                                              │
└──────────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│                    Discovery.provide(cid)                    │
│                                                              │
│  3. Create ProviderRecord                                    │
│     │                                                        │
│     ▼                                                        │
│     ProviderRecord {                                         │
│         cid: cid.to_string(),                                │
│         peer_id: self.peer_id.to_bytes(),                    │
│         addrs: self.announce_addrs.clone(),                  │
│         timestamp: SystemTime::now(),                        │
│     }                                                        │
│     │                                                        │
│     ▼                                                        │
│  4. Store locally (providers.add_local)                      │
│     │                                                        │
│     ▼                                                        │
│  5. Convert CID to NodeId (Keccak256)                        │
│     node_id = keccak256(cid.to_bytes())                      │
│     │                                                        │
│     ▼                                                        │
│  6. Find K closest DHT nodes                                 │
│     closest_nodes = discv5.find_node(node_id)                │
│     │                                                        │
│     ▼                                                        │
│  7. Send ADD_PROVIDER to top 3 nodes                         │
│     │                                                        │
│     ├─► For each node:                                       │
│     │    │                                                   │
│     │    ▼                                                   │
│     │    Serialize AddProviderRequest                        │
│     │    │                                                   │
│     │    ▼                                                   │
│     │    Send via TALK protocol                              │
│     │    │                                                   │
│     │    ├─► Success: Provider record stored on node         │
│     │    │   │                                               │
│     │    │   └─► Node can now serve this provider info       │
│     │    │                                                   │
│     │    └─► Failure: Log warning, continue                  │
│     │                                                        │
│     ▼                                                        │
│  8. Return Ok(())                                            │
│     Provider announcement complete ✅                         │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

## TALK Protocol Message Flow

### ADD_PROVIDER Request/Response

```
┌─────────────┐                              ┌─────────────┐
│  Provider   │                              │ DHT Node    │
│  (Sender)   │                              │ (Receiver)  │
└─────┬───────┘                              └─────┬───────┘
      │                                            │
      │  1. Find node closest to CID               │
      │     (via discv5.find_node)                 │
      ├────────────────────────────────────────────▶
      │                                            │
      │  2. Send ADD_PROVIDER request              │
      │     TALK protocol: "add_provider"          │
      │     Body: bincode(AddProviderRequest {     │
      │         record: ProviderRecord {...}       │
      │     })                                     │
      ├────────────────────────────────────────────▶
      │                                            │
      │                                            │ 3. Deserialize request
      │                                            │    │
      │                                            │    ▼
      │                                            │ 4. Validate CID
      │                                            │    │
      │                                            │    ▼
      │                                            │ 5. Store ProviderRecord
      │                                            │    providers.add_remote(cid, record)
      │                                            │    │
      │                                            │    ▼
      │  6. Respond with success                   │ 6. Create response
      │     Body: bincode(AddProviderResponse {    │    AddProviderResponse {
      │         success: true                      │        success: true
      │     })                                     │    }
      ◀────────────────────────────────────────────┤
      │                                            │
      ▼                                            ▼
Provider record                             Provider record
now advertised ✅                            now stored ✅
```

### GET_PROVIDERS Request/Response

```
┌─────────────┐                              ┌─────────────┐
│  Requester  │                              │ DHT Node    │
│  (Seeker)   │                              │ (Responder) │
└─────┬───────┘                              └─────┬───────┘
      │                                            │
      │  1. Convert CID to NodeId                  │
      │     node_id = keccak256(cid)               │
      │     │                                      │
      │     ▼                                      │
      │  2. Find nodes closest to NodeId           │
      │     (via discv5.find_node)                 │
      ├────────────────────────────────────────────▶
      │                                            │
      │  3. Send GET_PROVIDERS request             │
      │     TALK protocol: "get_providers"         │
      │     Body: bincode(GetProvidersRequest {    │
      │         cid: cid.to_string()               │
      │     })                                     │
      ├────────────────────────────────────────────▶
      │                                            │
      │                                            │ 4. Deserialize request
      │                                            │    │
      │                                            │    ▼
      │                                            │ 5. Parse CID
      │                                            │    │
      │                                            │    ▼
      │                                            │ 6. Lookup providers
      │                                            │    provider_records =
      │                                            │        providers.get_providers(cid)
      │                                            │    │
      │                                            │    ▼
      │  7. Receive provider list                  │ 7. Create response
      │     Body: bincode(GetProvidersResponse {   │    GetProvidersResponse {
      │         providers: Vec<ProviderRecord>,    │        providers: [...],
      │         closer_peers: Vec<NodeId>          │        closer_peers: [...]
      │     })                                     │    }
      ◀────────────────────────────────────────────┤
      │                                            │
      ▼                                            ▼
  8. Cache providers locally                  Provider lookup
     providers.add_remote(...)                 complete ✅
     │
     ▼
  9. Extract PeerIds
     peer_ids = providers.iter()
         .filter_map(|r| PeerId::from_bytes(&r.peer_id).ok())
         .collect()
     │
     ▼
 10. Return to BlockExc
     Block request can now be sent to discovered peers ✅
```

## Metrics Flow

```
┌─────────────────────────────────────────────────────────────┐
│                      Metrics Tracking                       │
└─────────────────────────────────────────────────────────────┘

Discovery Query Initiated
│
├─► metrics.discovery_query()
│   │
│   └─► neverust_discovery_queries_total++
│
▼
Discovery Result
│
├─► Providers Found
│   │
│   ├─► metrics.discovery_success()
│   │   │
│   │   └─► neverust_discovery_successes_total++
│   │
│   └─► Block Retrieved via Discovery
│       │
│       └─► metrics.block_from_discovery()
│           │
│           └─► neverust_blocks_from_discovery_total++
│
└─► No Providers (Max Retries)
    │
    └─► metrics.discovery_failure()
        │
        └─► neverust_discovery_failures_total++

Success Rate Calculation (Prometheus query):
│
└─► (neverust_discovery_successes_total / neverust_discovery_queries_total) * 100
    │
    └─► neverust_discovery_success_rate
```

## State Transitions

```
                     BlockRequest Created
                            │
                            ▼
                ┌──────────────────────┐
                │  pending_requests    │
                │  (waiting for block) │
                └──────────┬───────────┘
                           │
                ┌──────────┴──────────┐
                │                     │
                ▼                     ▼
    ┌──────────────────┐   ┌──────────────────┐
    │ Block delivered  │   │ No peers have    │
    │ by connected     │   │ block            │
    │ peer (standard)  │   │                  │
    └────────┬─────────┘   └────────┬─────────┘
             │                      │
             │                      ▼
             │           ┌──────────────────┐
             │           │ discovery_queue  │
             │           │ (retry_count: 0) │
             │           └────────┬─────────┘
             │                    │
             │          ┌─────────┴─────────┐
             │          │                   │
             │          ▼                   ▼
             │  ┌───────────────┐   ┌─────────────────┐
             │  │ Providers     │   │ No providers    │
             │  │ found         │   │ (retry++)       │
             │  └───────┬───────┘   └───────┬─────────┘
             │          │                   │
             │          ▼                   │
             │  ┌───────────────┐           │
             │  │ Block         │           │
             │  │ requested     │           │
             │  │ from          │           │
             │  │ discovered    │           │
             │  │ provider      │           │
             │  └───────┬───────┘           │
             │          │                   │
             │          ▼                   ▼
             │  ┌───────────────┐   ┌─────────────────┐
             │  │ Block         │   │ Max retries (3) │
             │  │ delivered     │   │ reached         │
             │  │ (discovery-   │   │                 │
             │  │ assisted)     │   └────────┬────────┘
             │  └───────┬───────┘            │
             │          │                    │
             └──────────┴────────────────────┘
                        │
                        ▼
             ┌──────────────────┐
             │ Request complete │
             │ Remove from      │
             │ pending_requests │
             └──────────────────┘
```

## Performance Timeline

```
t=0ms     Client calls request_block(cid)
│
├─► t=1ms     BlockExcClient checks local blockstore
│   │
│   └─► Cache hit ──► Return immediately (1ms total) ✅
│
└─► t=2ms     Block not in local store
    │
    ▼
t=3ms     Send BlockRequest to swarm
│
▼
t=5ms     BlockExcBehaviour broadcasts to peers
│
├─► Connected peer has block
│   │
│   └─► t=50-200ms  Standard BlockExc retrieval ✅
│
└─► No peers have block
    │
    ▼
t=10ms    Queue for discovery
│
▼
t=15ms    process_discovery_queue() triggered
│
▼
t=20ms    discovery.find(cid) called
│
├─► Cached providers
│   │
│   └─► t=25ms  Return cached PeerIds (fast path) ✅
│
└─► Not cached - DHT lookup required
    │
    ▼
t=30ms    Convert CID to NodeId (Keccak256)
│
▼
t=35ms    Find K closest nodes (DHT lookup)
│         discv5.find_node(node_id)
│
└─► t=85-235ms  DHT lookup latency (50-200ms)
    │
    ▼
t=85-235ms     Query top 3 nodes via TALK
│
├─► Node 1: GET_PROVIDERS request
│   └─► t=+20-100ms  (TALK protocol latency)
│
├─► Node 2: GET_PROVIDERS request
│   └─► t=+20-100ms  (parallel with Node 1)
│
└─► Node 3: GET_PROVIDERS request
    └─► t=+20-100ms  (parallel with Nodes 1-2)
    │
    ▼
t=125-335ms    Providers received, cached locally
│
▼
t=130-340ms    Queue RequestBlock for discovered providers
│
├─► Provider already connected
│   │
│   └─► t=180-540ms  Standard BlockExc retrieval ✅
│
└─► Provider not connected
    │
    ▼
t=135-345ms    TODO: Dial provider
│              (currently logged, not implemented)
│
└─► t=235-845ms  Dial + BlockExc retrieval ✅
    │            (100-500ms dial + 50-200ms BlockExc)
    │
    ▼
TOTAL LATENCY RANGES:
│
├─► Local cache hit:                ~1ms
├─► Standard BlockExc (connected):  50-200ms
├─► Discovery (cached providers):   ~180-540ms
└─► Discovery (DHT lookup + dial):  ~235-1000ms
```
