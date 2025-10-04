# Simplified Dockerfile using pre-built binary
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy pre-built binary
COPY target/release/neverust /usr/local/bin/neverust

# Create data directory
RUN mkdir -p /data

# Expose ports
EXPOSE 8070/tcp 8090/udp 8080/tcp

# Default command
ENTRYPOINT ["/usr/local/bin/neverust"]
CMD ["start", "--mode", "altruistic", "--listen-port", "8070", "--disc-port", "8090", "--api-port", "8080", "--data-dir", "/data"]
