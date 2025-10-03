# NEVAR-8: Implement Config File Loading (TOML)

**Phase**: 0 | **Status**: Todo | **Priority**: High

## Description
Add support for loading configuration from a TOML file. Merge config from CLI args, config file, and defaults with proper priority (CLI > config file > defaults).

## Acceptance Criteria
- [ ] Define Config struct with serde Deserialize
- [ ] Load config from {data_dir}/config.toml
- [ ] Merge with CLI overrides
- [ ] Create default config if missing
- [ ] Test config precedence
- [ ] Committed

## Relationships
- **Blocked by**: NEVAR-1
- **Relates to**: NEVAR-3 (extends CLI config)
- **Start after**: NEVAR-3

## Technical Notes
```toml
# config.toml example
[network]
listen_port = 8070
disc_port = 8090

[storage]
data_dir = "./data"

[logging]
level = "info"
```

## Time Estimate
20 minutes
