import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import sassDts from 'vite-plugin-sass-dts';
import { execSync } from 'node:child_process';

const host = process.env.TAURI_DEV_HOST;

function getBuildNumber() {
  if (process.env.NODE_ENV === 'development') {
    return 'In Dev';
  }

  try {
    return execSync('git rev-list --count HEAD', { encoding: 'utf8' }).trim();
  } catch {
    return 'In Dev';
  }
}

export default defineConfig(async () => ({
  define: {
    __APP_BUILD_NUMBER__: JSON.stringify(getBuildNumber()),
  },
  plugins: [
    react(),
    sassDts()
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
        protocol: 'ws',
        host,
        port: 1421,
      }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
}));
