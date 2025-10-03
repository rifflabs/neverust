# neverust
Archivist Storage node in Rust

This is an implementation of https://github.com/durability-labs/archivist-node
in Rust, using Rust-libp2p.

## Project Vision

Build a production-ready, high-performance Archivist Storage node with:
- Ridiculously fast P2P content distribution
- Comprehensive automated testing across all interfaces
- Multi-device UX validation (Web UI, CLI, API)
- Complete observability and operational readiness
- Phoenix-style phase-based development methodology

## Current Status

**Phase 0: COMPLETE** ✅ (Completed 2025-10-03)
- Working P2P node with libp2p
- TCP transport with Noise encryption + Yamux multiplexing
- Ping + Identify protocols operational
- CLI with configuration options
- Structured logging with tracing
- All tests passing

**Phase 1: IN PROGRESS** 🚧 (Days 1-2)
- Kademlia DHT integration
- Block storage (CID-based)
- REST API endpoints
- Health checks and metrics

See [ISSUES.md](./ISSUES.md) for complete roadmap (150 issues tracked)

---

# Development Methodology

This project follows the **Phoenix Testing Framework** - a comprehensive, phase-based approach to building production-ready systems with automated UX validation.

## Phases

### Phase 1: Evaluate
Assess the current state of the application across all interfaces:
- **Web UI**: Test across browsers, resolutions, and devices
- **CLI**: Validate command-line interface across shells and platforms
- **API**: Test HTTP/gRPC endpoints, WebSocket connections, P2P protocols
- **Performance**: Measure latency, throughput, resource usage

**Tools**: Playwright MCP, integration tests, benchmarks, `pal test`

### Phase 2: Playthrough
Create automated UX testers simulating real user interactions:
- **Multi-Device Testing**: Desktop (1080p, 4K), Mobile (iOS, Android), Tablet, TV, VR
- **Multi-Input Testing**: Mouse/keyboard, touch, gamepad, voice, CLI commands
- **Multi-Platform Testing**: Linux, macOS, Windows, Docker, Kubernetes
- **Real User Scenarios**: Common workflows, edge cases, failure scenarios

**Approach**: Use Playwright MCP for browser automation, spawn parallel test instances across device profiles, simulate actual input (touch gestures, gamepad controls, keyboard shortcuts).

### Phase 3: Record
Generate comprehensive documentation and recordings:
- **Screen Recordings**: Capture browser sessions with voiceovers and subtitles
- **CLI Recordings**: asciinema or ttyrec for terminal sessions
- **API Traces**: OpenTelemetry traces, request/response logs
- **Director's Report**: Executive summary of testing results, pass rates, infrastructure assessment
- **Features Report**: Production readiness scoring, gap analysis, roadmap

**Tools**: Playwright video capture, Piper TTS (voiceovers), FFmpeg (video processing), GPT-5 via Zen MCP (report generation)

### Phase 4: Suggest
Produce comprehensive UX improvement analysis:
- **Categorize Issues**: Critical (block production), High (degrade UX), Medium (polish), Low (nice-to-have)
- **User Journey Maps**: End-to-end flows with pain points highlighted
- **Implementation Roadmap**: Sequenced recommendations with dependencies, quality gates, success metrics
- **Impact vs Effort Analysis**: Quantify user impact and development effort

**Output**: UX Improvement Suggestions Report, Implementation Roadmap with quality gates

### Phase 5: Build
Implement improvements with TDD and continuous validation:
- **Quick Wins First**: Low-effort, high-impact fixes (Phase 0)
- **Critical Blockers**: Search, reliability, observability (Phase 1-3)
- **Polish & Advanced Features**: Input methods, performance, accessibility (Phase 4)
- **Quality Gates**: Pass/fail criteria between phases, no progression without meeting thresholds

---

# Useful Notes During Development

## MCP Tools Usage

* **DeepWiki MCP** - Use *exhaustively* for documentation research:
  * durability-labs/archivist-docs
  * durability-labs/archivist-node
  * libp2p/rust-libp2p
  * Any relevant Rust crates (tokio, serde, etc.)

* **Playwright MCP** - Use *liberally* for all browser/UI automation:
  * Multi-device testing (12+ device profiles)
  * Input simulation (touch, gamepad, keyboard)
  * Visual regression testing
  * Network request validation
  * Console error detection

* **Zen MCP** - Use for deep analysis and report generation:
  * `chat` - Brainstorming, second opinions, collaborative thinking
  * `thinkdeep` - Complex problem analysis, architecture decisions
  * `planner` - Sequential planning with revision and branching
  * `consensus` - Multi-model debate for complex decisions
  * `codereview` - Systematic code review with expert validation
  * `precommit` - Git change validation before commits
  * `debug` - Root cause analysis with hypothesis testing

* **Pal MCP** - Use `pal next --fast` liberally for task generation and planning

## Development Philosophy

* **Build so that you can reload the UI without recompiling the binary or refreshing a browser**
  - Lazy development is good development!
  - Hot module replacement for frontend
  - Watch mode for backend with auto-restart
  - API-first design allows UI/backend to evolve independently

* **Test-Driven Development Always**
  - Write tests before implementation (no exceptions)
  - Multi-device test matrix (Desktop, Mobile, Tablet, TV, VR)
  - Input method validation (Touch, Gamepad, Keyboard, Voice)
  - Performance benchmarks (latency, throughput, resource usage)

* **Phoenix-Style Reporting**
  - Director's Report: Executive summary, pass rates, infrastructure scoring
  - Features Report: Production readiness, gap analysis, roadmap
  - UX Improvement Suggestions: Categorized issues, user journeys, implementation plan
  - All reports generated via GPT-5 (Zen MCP) for comprehensive synthesis

## Multi-Device Testing Matrix

When testing UI components, use comprehensive device profiles:

| Device Type | Profiles | Resolution | Input Methods |
|-------------|----------|------------|---------------|
| Desktop     | 4        | 1080p (Chrome/Firefox/Safari), 4K | Mouse, Keyboard, Touch, Gamepad |
| Mobile      | 3        | iPhone 15, iPhone 15 Pro Max, Pixel 7 | Touch, Multi-touch, Orientation |
| Tablet      | 3        | iPad Pro, iPad Mini, Galaxy Tab S4 | Touch, Stylus, Keyboard |
| TV          | 1        | 1080p Chromecast | Remote, Gamepad, Voice |
| VR          | 1        | Quest 3 (1832x1920) | Gaze, Controller, Hand tracking |

**Total**: 12 device profiles, 876 tests (73 base tests × 12 devices)

## Input Method Testing

Test ALL input methods that users might employ:

### Touch Input (8 test categories)
- Tap gestures (single, double)
- Swipe gestures (horizontal, vertical, diagonal)
- Pinch/zoom
- Long-press
- Multi-touch (2+ fingers)
- Rotation handling
- Touch accessibility (target sizes, spacing)

### Gamepad Input (13 test categories)
- Gamepad API detection
- D-Pad navigation (arrow keys, directional buttons)
- Button mapping (A/B/X/Y, shoulder buttons, triggers)
- Analog stick input (left/right sticks, dead zones)
- Haptics/vibration (if supported)
- Gyro/accelerometer (6-axis controls)
- Button combinations (modifiers)

**Advanced**: WASM Gilrs integration for consistent cross-browser gamepad support

### Keyboard/Mouse Input
- Keyboard shortcuts (/, Escape, Tab, Arrow keys, Enter)
- Focus management (roving tabindex, spatial navigation)
- Mouse interactions (click, hover, drag, context menu)
- Accessibility (screen readers, keyboard-only navigation)

### Voice Input (if applicable)
- Voice commands
- Speech-to-text
- Voice feedback

## Observability & Operations

Every production system needs comprehensive observability:

### Health Checks
- `/health` endpoint for all services
- Readiness probes (dependencies available)
- Liveness probes (service responsive)
- Pre-flight infrastructure gating in CI

### Metrics (Prometheus-compatible)
- **P2P Metrics**: `peer_count`, `dial_ms` (p50/p95/p99), `content_fetch_ms`
- **API Metrics**: Request rate, latency, error rate
- **System Metrics**: CPU, memory, disk I/O, network bandwidth
- **Application Metrics**: Active connections, cache hit rate, queue depth

### Dashboards
- **Grafana**: Performance metrics, SLO tracking, alerting
- **Homarr**: Quick links to services, status overview
- **Jaeger/Tempo**: Distributed tracing for P2P operations

### Alerts
- Dial failure rate >1%
- Content fetch p95 >2.5s
- Peer count <2
- Error rate >0.1%
- Disk usage >80%

## Performance Targets

- **Initial Load**: <7s (including P2P initialization)
- **Peer Connection**: ~1s
- **Content Fetch**: 2-3s post-initialization
- **Navigation Transitions**: 1-2s
- **Search Latency**: p95 ≤150ms
- **API Response Time**: p95 ≤100ms

## Quality Standards

This project follows Palace best practices:
- ✅ Test-driven development (write tests FIRST, always)
- ✅ Comprehensive test coverage (>80% code coverage, 100% critical paths)
- ✅ Small, atomic commits (single logical change per commit)
- ✅ Clear documentation (README, CLAUDE.md, inline comments)
- ✅ Modular architecture (<200 LoC per file, focused modules)
- ✅ Multi-device validation (12+ device profiles)
- ✅ Input method testing (Touch, Gamepad, Keyboard, Voice)
- ✅ Phoenix-style reporting (Director's, Features, UX Suggestions, Roadmap)

See [CLAUDE.md](./CLAUDE.md) for detailed development guidelines.

---

## Quick Start

```bash
# Build
cargo build --release

# Run the node
cargo run -- start

# Run with custom options
cargo run -- start --data-dir ./my-data --listen-port 9000 --log-level debug

# Test
cargo test

# CLI help
cargo run -- start --help
```

### Example Output

```
INFO neverust: Starting Neverust node...
INFO neverust_core::p2p: Local peer ID: 12D3KooWJQgUiKtBcQWooTJhvP2degnM77ca64nggKzG7s9crnMs
INFO neverust_core::runtime: Node started with peer ID: 12D3KooWJQgUiKtBcQWooTJhvP2degnM77ca64nggKzG7s9crnMs
INFO neverust_core::runtime: Listening on TCP port 8070
INFO neverust_core::runtime: Listening on /ip4/127.0.0.1/tcp/8070
INFO neverust_core::runtime: Listening on /ip4/10.7.1.193/tcp/8070
```

---

## Project Structure

```
neverust/
├── src/              # Rust source code
├── tests/            # Integration tests
├── ui-tests/         # Playwright multi-device tests
├── docs/             # Documentation
├── reports/          # Phase 3 reports (Director's, Features, UX)
├── benchmarks/       # Performance benchmarks
└── .github/          # CI/CD workflows
```

---

## Contributing

1. Read CLAUDE.md for development guidelines
2. Follow Phoenix phases: Evaluate → Playthrough → Record → Suggest → Build
3. Write tests before implementation (TDD always)
4. Test across device profiles (use Playwright MCP)
5. Generate reports via Zen MCP (Director's, Features, UX)
6. Commit regularly with atomic changes

**Remember**: Every feature goes through all 5 Phoenix phases before being considered complete.
