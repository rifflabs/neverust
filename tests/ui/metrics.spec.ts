import { test, expect } from '@playwright/test';

/**
 * Prometheus metrics endpoint tests
 *
 * Tests the /metrics endpoint across all device profiles
 */

test.describe('Metrics Endpoint', () => {
  test('should return 200 OK status', async ({ page }) => {
    const response = await page.goto('/metrics');
    expect(response?.status()).toBe(200);
  });

  test('should return Prometheus text format', async ({ page }) => {
    const response = await page.goto('/metrics');
    const contentType = response?.headers()['content-type'];

    expect(contentType).toContain('text/plain');
  });

  test('should include all required metrics', async ({ page }) => {
    const response = await page.goto('/metrics');
    const body = await response?.text();

    // Core metrics
    expect(body).toContain('neverust_block_count');
    expect(body).toContain('neverust_block_bytes');
    expect(body).toContain('neverust_uptime_seconds');

    // P2P metrics
    expect(body).toContain('neverust_peer_connections');
    expect(body).toContain('neverust_total_peers_seen');

    // Transfer metrics
    expect(body).toContain('neverust_blocks_sent_total');
    expect(body).toContain('neverust_blocks_received_total');
    expect(body).toContain('neverust_bytes_sent_total');
    expect(body).toContain('neverust_bytes_received_total');

    // Cache metrics
    expect(body).toContain('neverust_cache_hits_total');
    expect(body).toContain('neverust_cache_misses_total');

    // Latency metrics
    expect(body).toContain('neverust_avg_exchange_time_ms');
  });

  test('should have valid Prometheus HELP and TYPE comments', async ({ page }) => {
    const response = await page.goto('/metrics');
    const body = await response?.text();

    expect(body).toContain('# HELP neverust_block_count');
    expect(body).toContain('# TYPE neverust_block_count gauge');
    expect(body).toContain('# HELP neverust_uptime_seconds');
    expect(body).toContain('# TYPE neverust_uptime_seconds counter');
  });

  test('should have numeric values for all metrics', async ({ page }) => {
    const response = await page.goto('/metrics');
    const body = await response?.text();

    // Extract metric lines (not HELP or TYPE)
    const metricLines = body
      .split('\n')
      .filter(line => line && !line.startsWith('#'));

    expect(metricLines.length).toBeGreaterThan(0);

    for (const line of metricLines) {
      const parts = line.split(' ');
      if (parts.length >= 2) {
        const value = parseFloat(parts[parts.length - 1]);
        expect(isNaN(value)).toBe(false);
      }
    }
  });

  test('should show increasing uptime on subsequent requests', async ({ page }) => {
    // First request
    const response1 = await page.goto('/metrics');
    const body1 = await response1?.text();
    const uptime1Match = body1.match(/neverust_uptime_seconds (\d+)/);
    const uptime1 = uptime1Match ? parseInt(uptime1Match[1]) : 0;

    // Wait 2 seconds
    await page.waitForTimeout(2000);

    // Second request
    const response2 = await page.goto('/metrics');
    const body2 = await response2?.text();
    const uptime2Match = body2.match(/neverust_uptime_seconds (\d+)/);
    const uptime2 = uptime2Match ? parseInt(uptime2Match[1]) : 0;

    expect(uptime2).toBeGreaterThanOrEqual(uptime1 + 2);
  });
});
