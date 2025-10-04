import { test, expect } from '@playwright/test';

/**
 * Health endpoint tests
 *
 * Tests the /health endpoint across all device profiles
 */

test.describe('Health Endpoint', () => {
  test('should return 200 OK status', async ({ page }) => {
    const response = await page.goto('/health');
    expect(response?.status()).toBe(200);
  });

  test('should return valid JSON with status field', async ({ page }) => {
    const response = await page.goto('/health');
    const body = await response?.json();

    expect(body).toHaveProperty('status');
    expect(body.status).toBe('ok');
  });

  test('should include block_count and total_bytes', async ({ page }) => {
    const response = await page.goto('/health');
    const body = await response?.json();

    expect(body).toHaveProperty('block_count');
    expect(body).toHaveProperty('total_bytes');
    expect(typeof body.block_count).toBe('number');
    expect(typeof body.total_bytes).toBe('number');
  });

  test('should respond within 1 second', async ({ page }) => {
    const startTime = Date.now();
    await page.goto('/health');
    const duration = Date.now() - startTime;

    expect(duration).toBeLessThan(1000);
  });
});
