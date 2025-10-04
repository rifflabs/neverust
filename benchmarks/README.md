# Neverust Benchmarking Suite

Comprehensive benchmarking infrastructure with real-time visualization for testing Neverust at scale.

## 🚀 Features

- **50-Node Cluster** - Configurable number of Neverust nodes
- **Real-time D3.js Visualization** - Force-directed graph showing network topology
- **Prometheus Metrics** - Comprehensive monitoring and metrics collection
- **Grafana Dashboards** - Pre-configured dashboards for performance analysis
- **Multi-mode Testing** - Mix of ALTRUISTIC and MARKETPLACE nodes
- **Live Resource Monitoring** - CPU, memory, disk, network usage

## 📊 Architecture

```
┌─────────────┐
│  Bootstrap  │ ◄─── Discovery hub for all nodes
└─────────────┘
       │
       ├─── Node 1 (Altruistic)
       ├─── Node 2 (Altruistic)
       ├─── Node 3 (Marketplace)
       ├─── Node 4 (Altruistic)
       └─── ... 50 nodes total

┌──────────────┐   ┌──────────┐   ┌─────────────┐
│  Prometheus  │──►│  Grafana │   │  Visualizer │
└──────────────┘   └──────────┘   └─────────────┘
```

## 🏃 Quick Start

### 1. Generate Docker Compose Configuration

```bash
cd benchmarks
python3 generate-compose.py 50  # Generate for 50 nodes (adjust as needed)
```

### 2. Build and Launch

```bash
# Build Neverust image
docker compose build

# Launch entire cluster
docker compose up -d

# View logs
docker compose logs -f bootstrap node1 node2

# Scale specific services
docker compose up -d --scale node1=10
```

### 3. Access Dashboards

- **Network Visualizer**: http://localhost:8888
- **Grafana**: http://localhost:3000
  - Username: `admin`
  - Password: `neverust`
  - ⚠️ **Testing only** - Change password in production!
- **Prometheus**: http://localhost:9090
- **Bootstrap API**: http://localhost:9080

## 📈 Metrics Exposed

Every Neverust node exposes Prometheus metrics at `/metrics`:

```
# HELP neverust_block_count Total number of blocks stored
# TYPE neverust_block_count gauge
neverust_block_count 42

# HELP neverust_block_bytes Total bytes of block data stored
# TYPE neverust_block_bytes gauge
neverust_block_bytes 1048576

# HELP neverust_uptime_seconds Time since node started
# TYPE neverust_uptime_seconds counter
neverust_uptime_seconds 3600
```

## 🧪 Running Benchmarks

### Upload Blocks to Random Nodes

```bash
# Upload test data to first 5 nodes
for i in {1..5}; do
  curl -X POST "http://localhost:$(($i + 9080))/api/archivist/v1/data" \
    -H 'Content-Type: application/octet-stream' \
    --data "Block data from benchmark test $i"
done
```

### Measure Block Exchange Speed

```bash
# Upload to node1
CID=$(curl -X POST http://localhost:9081/api/archivist/v1/data \
  -H 'Content-Type: application/octet-stream' \
  --data "Test block for exchange benchmark" 2>/dev/null)

# Download from node2 (tests peer exchange)
time curl "http://localhost:9082/api/archivist/v1/data/${CID}/network/stream" \
  -o /dev/null -s
```

### Stress Test with Parallel Uploads

```bash
# 100 parallel uploads across 5 nodes
seq 1 100 | xargs -P 10 -I {} bash -c '
  NODE=$((RANDOM % 5 + 1))
  curl -X POST "http://localhost:$(($NODE + 9080))/api/archivist/v1/data" \
    -H "Content-Type: application/octet-stream" \
    --data "Block {}" -s
'
```

## 📊 Visualization Dashboard

The D3.js visualizer shows:

- **Node Types**:
  - 🔴 Bootstrap (red)
  - 🔵 Altruistic (blue)
  - 🟣 Marketplace (purple)

- **Real-time Stats**:
  - Active nodes count
  - Total blocks stored
  - Total data size
  - Transfer speeds

- **Interactive Features**:
  - Drag nodes to rearrange
  - Hover for node details
  - Auto-updating every 2 seconds

## 🔧 Configuration

### Adjust Node Count

```bash
# Generate for different cluster sizes
python3 generate-compose.py 10   # Small cluster
python3 generate-compose.py 50   # Medium cluster
python3 generate-compose.py 100  # Large cluster
```

### Customize Node Mix

Edit `generate-compose.py`:

```python
# Line 62 - Change marketplace ratio
'--mode', 'altruistic' if i % 3 != 0 else 'marketplace',  # Current: 1/3 marketplace

# Change to:
'--mode', 'marketplace' if i % 2 == 0 else 'altruistic',  # 50% marketplace
```

### Resource Limits

Add to docker-compose.yml services:

```yaml
deploy:
  resources:
    limits:
      cpus: '0.5'
      memory: 256M
    reservations:
      cpus: '0.25'
      memory: 128M
```

## 📉 Performance Benchmarks

Expected performance metrics:

### Block Exchange Speed
- **Local cluster**: <10ms per block
- **With TGP compression**: 5-15x faster reconciliation
- **50 nodes**: ~100-500 blocks/sec aggregate throughput

### Resource Usage (per node)
- **Memory**: 50-100MB baseline
- **CPU**: <5% idle, 20-40% under load
- **Disk**: Minimal (in-memory cache by default)

### Network Bandwidth
- **P2P traffic**: ~1-10 MB/s per active node
- **Metrics**: ~1 KB/s per node (negligible)

## 🧹 Cleanup

```bash
# Stop all services
docker compose down

# Remove volumes (clears data)
docker compose down -v

# Remove images
docker compose down --rmi all -v
```

## 🎯 Next Steps

1. **Add Multi-tier Cache** - Memory → NVMe → SSD → HDD
2. **TGP Wantlist Compression** - Fast state reconciliation
3. **Advanced Benchmarks** - Latency, throughput, scalability tests
4. **Chaos Testing** - Random node failures, network partitions

## 📝 Notes

- Bootstrap node is always on `172.25.0.10`
- Worker nodes get IPs `172.25.1.1` through `172.25.X.X`
- Prometheus scrapes all nodes every 5s
- Visualizer updates every 2s
- First 5 worker nodes expose API on ports 9081-9085

## 🐛 Troubleshooting

**Nodes not connecting?**
```bash
# Check bootstrap health
curl http://localhost:9080/health

# View bootstrap logs
docker compose logs bootstrap

# Check network connectivity
docker compose exec node1 ping bootstrap
```

**Metrics not showing?**
```bash
# Verify Prometheus targets
curl http://localhost:9090/api/v1/targets | jq

# Check node metrics endpoint
curl http://localhost:9081/metrics
```

**Visualizer not updating?**
```bash
# Check visualizer logs
docker compose logs visualizer

# Verify WebSocket connection (browser console)
# Should see: "Connected to visualizer"
```
