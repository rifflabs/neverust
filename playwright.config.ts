import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for Neverust multi-device testing
 *
 * Tests across 12 device profiles:
 * - 4 Desktop (Chromium, Firefox, WebKit, 4K)
 * - 3 Mobile (iPhone 15, iPhone 15 Pro Max, Pixel 7)
 * - 3 Tablet (iPad Pro, iPad Mini, Galaxy Tab S4)
 * - 1 TV (1080p Chromecast)
 * - 1 VR (Quest 3)
 *
 * Total: 73 base tests × 12 devices = 876 test executions
 */
export default defineConfig({
  testDir: './tests/ui',

  // Test execution settings
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 4 : 8,

  // Reporter configuration
  reporter: [
    ['html', { outputFolder: 'playwright-report' }],
    ['json', { outputFile: 'test-results/results.json' }],
    ['junit', { outputFile: 'test-results/junit.xml' }],
    ['list'],
  ],

  // Global test settings
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:8080',
    trace: 'retain-on-failure',
    video: 'retain-on-failure',
    screenshot: 'only-on-failure',
    actionTimeout: 10000,
    navigationTimeout: 30000,
  },

  // Device matrix: 12 profiles
  projects: [
    // ===== Desktop Browsers (4) =====
    {
      name: 'desktop-1080p-chromium',
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1920, height: 1080 },
      },
    },
    {
      name: 'desktop-1080p-firefox',
      use: {
        ...devices['Desktop Firefox'],
        viewport: { width: 1920, height: 1080 },
      },
    },
    {
      name: 'desktop-1080p-webkit',
      use: {
        ...devices['Desktop Safari'],
        viewport: { width: 1920, height: 1080 },
      },
    },
    {
      name: 'desktop-4k-chromium',
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 3840, height: 2160 },
        deviceScaleFactor: 2,
      },
    },

    // ===== Mobile Devices (3) =====
    {
      name: 'iphone-15',
      use: {
        ...devices['iPhone 15'],
        // 393x852, deviceScaleFactor: 3
      },
    },
    {
      name: 'iphone-15-pro-max',
      use: {
        ...devices['iPhone 15 Pro Max'],
        // 430x932, deviceScaleFactor: 3
      },
    },
    {
      name: 'pixel-7',
      use: {
        ...devices['Pixel 7'],
        // 412x915, deviceScaleFactor: 2.625
      },
    },

    // ===== Tablets (3) =====
    {
      name: 'ipad-pro',
      use: {
        ...devices['iPad Pro'],
        // 1024x1366, deviceScaleFactor: 2
      },
    },
    {
      name: 'ipad-mini',
      use: {
        ...devices['iPad Mini'],
        // 768x1024, deviceScaleFactor: 2
      },
    },
    {
      name: 'galaxy-tab-s4',
      use: {
        ...devices['Galaxy Tab S4'],
        // 712x1138, deviceScaleFactor: 2.25
      },
    },

    // ===== TV (1) =====
    {
      name: 'tv-1080p',
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1920, height: 1080 },
        deviceScaleFactor: 1,
        hasTouch: false,
        userAgent: 'Mozilla/5.0 (Linux; Android 9; Chromecast) AppleWebKit/537.36',
      },
    },

    // ===== VR (1) =====
    {
      name: 'vr-quest-3',
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1832, height: 1920 },
        deviceScaleFactor: 1,
        userAgent: 'Mozilla/5.0 (Linux; Android 12; Quest 3) AppleWebKit/537.36',
      },
    },
  ],

  // Web server for local testing
  webServer: {
    command: 'cargo run --release -- start --api-port 8080',
    url: 'http://localhost:8080/health',
    timeout: 120000,
    reuseExistingServer: !process.env.CI,
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
