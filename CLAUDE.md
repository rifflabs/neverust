# Guidelines for Claude Code

## Project Overview

**Neverust** is a high-performance Archivist Storage node implementation in Rust using rust-libp2p. This project aims to create a production-ready, blazingly fast P2P content distribution system with comprehensive automated testing, multi-device UX validation, and complete observability.

**Key Goals**:
- Ridiculously fast P2P content distribution (sub-3s content fetch post-init)
- Production-grade reliability and observability
- Comprehensive multi-device testing (Desktop, Mobile, Tablet, TV, VR)
- Phoenix-style phase-based development (Evaluate → Playthrough → Record → Suggest → Build)
- Hot-reload development experience (UI changes without binary recompilation)

**Architecture**:
- Rust backend with rust-libp2p for P2P networking
- Web UI for browser-based management (hot-reloadable)
- CLI for command-line operations
- REST/gRPC API for programmatic access
- OpenTelemetry integration for distributed tracing

---

## Rules

- **Consult README.md** for context whenever needed - it contains the Phoenix Testing Framework phases and comprehensive guidance
- **Test Driven Development** - Write tests before implementing ANY code or feature, no matter how small. We aim for high code coverage from the beginning.
- **Zero Placeholders** - Do not put in references to commands or functionality that are not implemented yet or do not exist
- **Modularity** - Break down components into small, focused files (typically <200 LoC per file)
- **Test Modularity** - Tests should be modular and organized for easy understanding and maintenance
- **"DO NOT SIMPLIFY - EVER"** - When thinking of simplifying something, think through the change deeply and ask the user what they want to do
- **Commit Regularly** - Test after every change and commit very regularly with tiny atomic chunks
- **Follow Language Style Guides** - Adhere to Rust style guide (rustfmt, clippy)
- **Use Palace Tools** - Use `pal test`, `pal build`, `pal run` for development workflows

---

## Quality Standards

- Write comprehensive tests for all new features
- Keep functions small and focused (<50 lines typically)
- Use meaningful variable and function names (Rust naming conventions)
- Document complex logic with clear comments
- Handle errors gracefully with proper error messages (use `thiserror` or `anyhow`)
- **Code Coverage**: Aim for >80% overall, 100% for critical paths
- **Multi-Device Validation**: Test UI across 12+ device profiles
- **Input Method Testing**: Validate touch, gamepad, keyboard, voice
- **Performance Benchmarks**: Track latency, throughput, resource usage
- **Phoenix Reporting**: Generate Director's Report, Features Report, UX Suggestions, Implementation Roadmap

---

## Development Workflow

### Standard TDD Cycle

1. **Understand Requirements** - Read README.md and existing code
2. **Write Tests First** - Create failing tests that define expected behavior
3. **Implement Features** - Write minimal code to make tests pass
4. **Refactor** - Clean up code while keeping tests green
5. **Commit** - Small, atomic commits with clear messages

### Phoenix Phase-Based Workflow

Every feature goes through all 5 phases:

#### Phase 1: Evaluate
- Assess current state across all interfaces (Web UI, CLI, API)
- Run baseline tests (unit, integration, E2E)
- Measure performance benchmarks
- **Tools**: `pal test`, Playwright MCP, benchmarks

#### Phase 2: Playthrough
- Create automated UX testers simulating real user interactions
- Test across 12+ device profiles (Desktop, Mobile, Tablet, TV, VR)
- Validate all input methods (Touch, Gamepad, Keyboard, Voice)
- **Tools**: Playwright MCP (multi-device), parallel test execution

#### Phase 3: Record
- Generate screen recordings with voiceovers (Piper TTS + FFmpeg)
- Produce Director's Report (GPT-5 via Zen MCP)
- Produce Features Report (GPT-5 via Zen MCP)
- **Outputs**: Video demos, Executive summaries, Production readiness assessment

#### Phase 4: Suggest
- Categorize issues (Critical, High, Medium, Low)
- Map user journeys with pain points
- Create implementation roadmap with quality gates
- **Outputs**: UX Improvement Suggestions Report, Implementation Roadmap

#### Phase 5: Build
- Implement improvements following roadmap
- Pass quality gates between phases
- Validate with comprehensive testing
- **Approach**: Quick wins first, then critical blockers, then polish

---

## Palace Integration

This project uses Palace (`pal`) for development:

- `pal test` - Run tests (unit, integration, E2E)
- `pal build` - Build the project (Rust + UI assets)
- `pal run` - Run the project with hot-reload
- `pal next` - Get AI suggestions for next tasks
- `pal next --fast` - Quick task generation for planning
- `pal commit` - Create well-formatted commits
- `pal switch` - Switch between development machines

---

## Project-Specific Guidelines

### Rust Best Practices

- **Error Handling**: Use `Result<T, E>` everywhere; prefer `thiserror` for library errors, `anyhow` for applications
- **Async Runtime**: Use `tokio` for async operations
- **Serialization**: Use `serde` with `serde_json` for JSON, `bincode` for binary
- **Logging**: Use `tracing` with structured logging (not `log` crate)
- **Testing**: Use `#[tokio::test]` for async tests
- **Benchmarking**: Use `criterion` for performance benchmarks
- **Linting**: Run `cargo clippy` before commits; enforce with CI
- **Formatting**: Run `cargo fmt` before commits; enforce with CI

### P2P Networking (rust-libp2p)

- **Protocols**: Implement custom protocols using `RequestResponse` or `Gossipsub`
- **Peer Discovery**: Use `mdns`, `kad-dht`, or bootstrap nodes
- **Metrics**: Track `peer_count`, `dial_ms`, `content_fetch_ms` via Prometheus
- **Testing**: Use `libp2p-swarm-test` for P2P protocol testing
- **Reliability**: Implement retry logic with exponential backoff
- **Performance**: Measure dial latency (p50/p95/p99), content fetch time

### Web UI Development

- **Framework**: Use modern web framework (React, Vue.js, Svelte)
- **Build Tool**: Use Vite or similar for hot module replacement
- **API Client**: Generate client from OpenAPI spec (or use tRPC)
- **State Management**: Use React Context, Zustand, or Pinia
- **Testing**: Use Playwright MCP for multi-device E2E testing
- **Hot Reload**: Build UI separately from Rust binary; serve via static file server or embedded assets

### CLI Development

- **Argument Parsing**: Use `clap` with derive macros
- **Terminal UI**: Use `ratatui` (formerly tui-rs) for interactive UIs
- **Progress**: Use `indicatif` for progress bars
- **Colors**: Use `colored` or `owo-colors` for terminal colors
- **Testing**: Use `assert_cmd` for CLI integration tests

### Observability

- **Tracing**: Use `tracing` with `tracing-subscriber` for structured logging
- **Metrics**: Use `prometheus` crate; expose `/metrics` endpoint
- **Health Checks**: Implement `/health` endpoint with readiness/liveness
- **Distributed Tracing**: Use `tracing-opentelemetry` for OpenTelemetry integration
- **Dashboards**: Provide Grafana dashboard JSON in `docs/grafana/`
- **Alerts**: Define alert rules in `docs/alerts/`

### Multi-Device Testing

Use Playwright MCP to test across comprehensive device matrix:

**Device Profiles** (12 total):
1. Desktop 1080p (Chromium) - 1920x1080
2. Desktop 1080p (Firefox) - 1920x1080
3. Desktop 1080p (WebKit/Safari) - 1920x1080
4. Desktop 4K (Chromium) - 3840x2160
5. iPhone 15 - 393x852
6. iPhone 15 Pro Max - 430x932
7. Pixel 7 - 412x915
8. iPad Pro - 1024x1366
9. iPad Mini - 768x1024
10. Galaxy Tab S4 - 712x1138
11. TV 1080p (Chromecast) - 1920x1080
12. VR Quest 3 - 1832x1920

**Test Categories** (73 base tests × 12 devices = 876 total):
- Core functionality (navigation, content loading, playback)
- Touch input (tap, swipe, pinch, long-press, multi-touch)
- Gamepad input (D-Pad, buttons, analog sticks, haptics, gyro)
- Keyboard navigation (shortcuts, focus management, accessibility)
- Voice input (if applicable)
- Performance (load times, transitions, responsiveness)
- Visual regression (screenshot comparisons)

**Playwright Configuration**:
```typescript
// playwright.config.ts
export default {
  projects: [
    { name: 'desktop-1080p-chromium', use: { ...devices['Desktop Chrome'] } },
    { name: 'desktop-1080p-firefox', use: { ...devices['Desktop Firefox'] } },
    { name: 'desktop-1080p-webkit', use: { ...devices['Desktop Safari'] } },
    { name: 'desktop-4k-chromium', use: { viewport: { width: 3840, height: 2160 } } },
    { name: 'iphone-15', use: { ...devices['iPhone 15'] } },
    { name: 'iphone-15-pro-max', use: { ...devices['iPhone 15 Pro Max'] } },
    { name: 'pixel-7', use: { ...devices['Pixel 7'] } },
    { name: 'ipad-pro', use: { ...devices['iPad Pro'] } },
    { name: 'ipad-mini', use: { ...devices['iPad Mini'] } },
    { name: 'galaxy-tab-s4', use: { ...devices['Galaxy Tab S4'] } },
    { name: 'tv-1080p', use: { viewport: { width: 1920, height: 1080 } } },
    { name: 'vr-quest-3', use: { viewport: { width: 1832, height: 1920 } } },
  ],
  workers: 8, // Parallel execution
  retries: 0, // Local: no retries; CI: retries: 1
  use: {
    trace: 'retain-on-failure',
    video: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
};
```

### Input Method Testing

#### Touch Input Tests (8 categories)
```typescript
// tests/touch-input.spec.ts
test('should support tap gesture on content cards', async ({ page }) => {
  await page.locator('[data-testid="content-card"]').first().tap();
  // Assert navigation or modal open
});

test('should support swipe gesture on carousel', async ({ page }) => {
  const carousel = page.locator('[data-testid="carousel"]');
  await carousel.swipe({ direction: 'left', distance: 300 });
  // Assert carousel moved
});

test('should support pinch-to-zoom', async ({ page }) => {
  await page.touchscreen.pinch({ scale: 2.0 });
  // Assert zoom applied
});
```

#### Gamepad Input Tests (13 categories)
```typescript
// tests/gamepad-input.spec.ts
test('should detect gamepad API support', async ({ page }) => {
  const hasGamepadAPI = await page.evaluate(() => 'getGamepads' in navigator);
  expect(hasGamepadAPI).toBe(true);
});

test('should handle D-Pad navigation', async ({ page }) => {
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowRight');
  // Assert focus moved correctly
});

test('should handle gamepad haptics', async ({ page }) => {
  // WASM Gilrs integration for true gamepad support
  const hasVibration = await page.evaluate(() => {
    const gamepads = navigator.getGamepads();
    return gamepads?.[0]?.vibrationActuator ? true : false;
  });
  expect(typeof hasVibration).toBe('boolean');
});
```

### Performance Benchmarks

Track critical performance metrics:

```rust
// benches/p2p_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_peer_dial(c: &mut Criterion) {
    c.bench_function("peer_dial", |b| {
        b.iter(|| {
            // Benchmark peer dial time
            black_box(dial_peer())
        })
    });
}

fn bench_content_fetch(c: &mut Criterion) {
    c.bench_function("content_fetch", |b| {
        b.iter(|| {
            // Benchmark content fetch time
            black_box(fetch_content())
        })
    });
}

criterion_group!(benches, bench_peer_dial, bench_content_fetch);
criterion_main!(benches);
```

**Performance Targets**:
- Peer dial: p95 ≤1s
- Content fetch: p95 ≤2.5s (post-initialization)
- API response: p95 ≤100ms
- Search latency: p95 ≤150ms

### Phoenix Reporting with Zen MCP

Generate comprehensive reports using GPT-5 via Zen MCP:

#### Director's Report
```typescript
// Generate via Zen MCP chat tool
await mcp_zen_chat({
  prompt: `Generate Director's Report for neverust testing results...`,
  model: 'openai/gpt-5',
  files: [
    './test-results.json',
    './performance-benchmarks.json',
    './playwright-report/index.html'
  ]
});
```

**Contents**:
- Executive summary
- Pass rates per device profile
- Infrastructure assessment (scoring)
- Key findings and recommendations
- Timeline and budget estimates

#### Features Report
```typescript
// Generate via Zen MCP chat tool with continuation
await mcp_zen_chat({
  prompt: `Generate Features Report assessing production readiness...`,
  model: 'openai/gpt-5',
  continuation_id: '<from Director\'s Report>',
  files: [
    './reports/directors-report.md',
    './src/**/*.rs',
    './ui-tests/**/*.spec.ts'
  ]
});
```

**Contents**:
- Overall production readiness score
- Infrastructure readiness (breakdown)
- P2P architecture maturity
- Feature completeness vs vision
- Gap analysis (prioritized)
- 3/6/12 month roadmap

#### UX Improvement Suggestions
```typescript
// Generate via Zen MCP chat tool
await mcp_zen_chat({
  prompt: `Create comprehensive UX improvement suggestions report...`,
  model: 'openai/gpt-5',
  files: [
    './reports/directors-report.md',
    './reports/features-report.md',
    './reports/ux-assessment.md'
  ]
});
```

**Contents**:
- Categorized issues (Critical, High, Medium, Low)
- User journey analysis
- Input method improvements
- Performance optimization opportunities
- Accessibility enhancements
- Success metrics per feature

#### Implementation Roadmap
```typescript
// Generate via Zen MCP planner tool
await mcp_zen_planner({
  step: `Create detailed implementation plan for UX improvements...`,
  model: 'openai/gpt-5',
  step_number: 1,
  total_steps: 3,
  next_step_required: true
});
```

**Contents**:
- 5-phase sequential plan (Phase 0-4)
- Dependency visualization (ASCII diagram)
- Quality gates between phases
- Resource allocation scenarios
- Risk mitigation strategies
- Expected outcomes and success criteria

---

## Tool Integration

### DeepWiki MCP

Use exhaustively for documentation research:
```typescript
await mcp_deepwiki_read_wiki_structure({ repoName: 'durability-labs/archivist-docs' });
await mcp_deepwiki_read_wiki_contents({ repoName: 'durability-labs/archivist-node' });
await mcp_deepwiki_ask_question({
  repoName: 'libp2p/rust-libp2p',
  question: 'How do I implement custom request-response protocols?'
});
```

### Playwright MCP

Use liberally for all UI automation:
```typescript
// Navigate and snapshot
await mcp_playwright_browser_navigate({ url: 'http://localhost:8000' });
const snapshot = await mcp_playwright_browser_snapshot({});

// Interact with elements
await mcp_playwright_browser_click({
  element: 'Content card',
  ref: '<ref from snapshot>'
});

// Type and submit
await mcp_playwright_browser_type({
  element: 'Search input',
  ref: '<ref>',
  text: 'test query',
  submit: true
});

// Take screenshots
await mcp_playwright_browser_take_screenshot({
  filename: 'homepage.png',
  fullPage: true
});
```

### Zen MCP

Use for deep analysis and synthesis:

- **chat**: Brainstorming, second opinions, collaborative thinking
- **thinkdeep**: Complex problem analysis, architecture decisions, root cause analysis
- **planner**: Sequential planning with revision, branching, dependency mapping
- **consensus**: Multi-model debate for complex decisions
- **codereview**: Systematic code review with expert validation
- **precommit**: Git change validation before commits
- **debug**: Root cause analysis with hypothesis testing

### Pal MCP

```bash
# Generate planning tasks
pal next --fast

# Execute pal commands
pal test
pal build
pal run
pal commit
```

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
# .github/workflows/test.yml
name: Test
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      # Rust tests
      - run: cargo test --all-features
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check

      # Benchmarks (don't fail on regression yet)
      - run: cargo bench --no-run

      # UI tests (multi-device)
      - run: npx playwright install
      - run: npx playwright test --project=desktop-1080p-chromium

      # Code coverage
      - uses: taiki-e/install-action@cargo-llvm-cov
      - run: cargo llvm-cov --all-features --codecov --output-path codecov.json
      - uses: codecov/codecov-action@v3
```

### Test Sharding for CI

```typescript
// playwright.config.ci.ts
export default {
  projects: [
    // Shard 1: Desktop only (fast feedback)
    { name: 'desktop-shard', testMatch: /.*desktop.*\.spec\.ts/ },

    // Shard 2: Mobile only
    { name: 'mobile-shard', testMatch: /.*mobile.*\.spec\.ts/ },

    // Shard 3: Input methods
    { name: 'input-shard', testMatch: /.*(touch|gamepad).*\.spec\.ts/ },
  ],
  retries: 1, // Enable retries in CI
  workers: 4,
};
```

---

## Success Metrics

Track these KPIs to measure progress:

### Code Quality
- **Test Coverage**: >80% overall, 100% critical paths
- **Build Time**: <5 minutes for full rebuild
- **Clippy Warnings**: 0
- **Documentation Coverage**: >70% (public APIs fully documented)

### Performance
- **Peer Dial**: p95 ≤1s
- **Content Fetch**: p95 ≤2.5s (post-init)
- **API Response**: p95 ≤100ms
- **Binary Size**: <50MB release build
- **Memory Usage**: <100MB idle, <500MB under load

### Testing
- **Test Pass Rate**: ≥97% overall (target from Phoenix: 850/876)
- **Desktop Pass Rate**: 100% (critical)
- **Mobile Pass Rate**: ≥96%
- **TV Pass Rate**: ≥95%
- **VR Pass Rate**: ≥90%
- **CI Duration**: ≤10 minutes (with sharding)

### Production Readiness
- **Overall Readiness**: 90%+ (Phoenix target)
- **Infrastructure Score**: 90/100
- **Feature Completeness**: 85/100
- **P2P Maturity**: 80/100
- **Observability**: 100% (all services monitored)

---

## Common Patterns

### Error Handling Pattern
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArchivistError {
    #[error("Peer dial failed: {0}")]
    DialFailed(String),

    #[error("Content fetch timeout after {0}ms")]
    FetchTimeout(u64),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ArchivistError>;
```

### Metrics Pattern
```rust
use prometheus::{Registry, IntGauge, Histogram};

pub struct Metrics {
    pub peer_count: IntGauge,
    pub dial_duration: Histogram,
    pub fetch_duration: Histogram,
}

impl Metrics {
    pub fn new(registry: &Registry) -> Self {
        let peer_count = IntGauge::new("peer_count", "Number of connected peers").unwrap();
        let dial_duration = Histogram::new("dial_duration_ms", "Peer dial latency").unwrap();
        let fetch_duration = Histogram::new("fetch_duration_ms", "Content fetch latency").unwrap();

        registry.register(Box::new(peer_count.clone())).unwrap();
        registry.register(Box::new(dial_duration.clone())).unwrap();
        registry.register(Box::new(fetch_duration.clone())).unwrap();

        Self { peer_count, dial_duration, fetch_duration }
    }
}
```

### Tracing Pattern
```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(self), fields(peer_id = %peer_id))]
async fn dial_peer(&self, peer_id: PeerId) -> Result<()> {
    info!("Attempting to dial peer");

    let start = Instant::now();
    match self.swarm.dial(peer_id).await {
        Ok(_) => {
            let duration = start.elapsed();
            self.metrics.dial_duration.observe(duration.as_millis() as f64);
            info!(duration_ms = duration.as_millis(), "Peer dial successful");
            Ok(())
        }
        Err(e) => {
            error!(error = %e, "Peer dial failed");
            Err(ArchivistError::DialFailed(e.to_string()))
        }
    }
}
```

---

## Final Checklist Before Committing

- [ ] All tests pass (`cargo test`, `npx playwright test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Documentation updated (if public API changed)
- [ ] Multi-device tests run (at least desktop-1080p-chromium)
- [ ] Performance benchmarks run (if performance-critical code changed)
- [ ] Commit message follows convention (conventional commits)
- [ ] Changes are atomic (single logical change)
- [ ] No placeholders or TODOs in committed code

---

**Remember**: Every feature goes through all 5 Phoenix phases. Never skip testing. Always measure performance. Ship with confidence.
