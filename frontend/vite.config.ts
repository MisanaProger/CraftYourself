import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// In development the Vite server proxies the API to the local daemon
// (`est serve`). The daemon rejects cross-origin requests, so the proxy
// drops the browser's Origin header: to the daemon the request then looks
// like it came from its own page.
const daemon = process.env.EST_DAEMON ?? "http://127.0.0.1:7878";

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/api": {
        target: daemon,
        changeOrigin: true,
        ws: true,
        configure: (proxy) => {
          proxy.on("proxyReq", (req) => req.removeHeader("origin"));
          proxy.on("proxyReqWs", (req) => req.removeHeader("origin"));
        },
      },
    },
  },
});
