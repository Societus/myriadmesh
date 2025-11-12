# MyriadNode Web UI

A lightweight, privacy-focused dashboard for managing and monitoring MyriadNode mesh networks.

Built with **SvelteKit** for minimal bundle size (~3KB runtime) and excellent performance.

## Features

- 📊 **Real-time Dashboard** - Monitor node status, adapters, and network health
- 🔌 **Adapter Management** - Start, stop, and configure network adapters
- 🗺️ **NodeMap Visualization** - View discovered nodes in the mesh network
- 🔄 **Automatic Failover** - Monitor and control adapter failover
- 🔐 **Privacy-First** - No telemetry, all data stays local
- ⚡ **Lightweight** - Tiny bundle size, fast loading

## Prerequisites

- Node.js 18+ and npm
- MyriadNode backend running on `http://127.0.0.1:8080`

## Installation

```bash
cd crates/myriadnode/web-ui
npm install
```

## Development

Start the development server with hot-reload:

```bash
npm run dev
```

The dashboard will be available at `http://localhost:5173`

During development, API requests to `/api/*` are automatically proxied to the MyriadNode backend at `http://127.0.0.1:8080`.

## Building for Production

Build the static site:

```bash
npm run build
```

The built site will be in the `build/` directory. You can preview it with:

```bash
npm run preview
```

## Deployment

The Web UI is a static SPA (Single Page Application) that can be deployed anywhere:

### Option 1: Serve from MyriadNode (Recommended)

The MyriadNode REST API can serve the static files:

1. Build the UI: `npm run build`
2. Copy `build/*` to MyriadNode's static assets directory
3. Access at `http://127.0.0.1:8080/`

### Option 2: Separate Web Server

Deploy the `build/` directory to any static hosting:

- Nginx
- Apache
- Caddy
- Netlify / Vercel / Cloudflare Pages

Configure your web server to proxy `/api/*` to `http://127.0.0.1:8080/api/`

## Project Structure

```
web-ui/
├── src/
│   ├── lib/
│   │   ├── api.ts              # REST API client
│   │   ├── stores.ts           # Svelte stores (global state)
│   │   ├── dataService.ts      # Data fetching and polling
│   │   └── components/
│   │       ├── AdapterCard.svelte
│   │       └── StatCard.svelte
│   ├── routes/
│   │   ├── +layout.svelte      # Main layout with sidebar
│   │   ├── +page.svelte        # Dashboard (/)
│   │   ├── adapters/
│   │   │   └── +page.svelte    # Adapters management
│   │   ├── nodemap/
│   │   │   └── +page.svelte    # NodeMap visualization
│   │   └── settings/
│   │       └── +page.svelte    # Settings
│   └── app.html                # HTML template
├── package.json
├── svelte.config.js
├── vite.config.js
└── tsconfig.json
```

## Configuration

### API Endpoint

The API endpoint is configured in `vite.config.js`:

```js
server: {
  proxy: {
    '/api': {
      target: 'http://127.0.0.1:8080',
      changeOrigin: true
    }
  }
}
```

### Auto-refresh Interval

The dashboard polls the backend every 5 seconds by default. This can be changed in the Settings page or by modifying `src/lib/dataService.ts`.

## API Requirements

The Web UI expects the following REST API endpoints from MyriadNode:

### Node Information
- `GET /api/node/info` - Node metadata and uptime

### Adapters
- `GET /api/adapters` - List all adapters
- `GET /api/adapters/:id` - Get adapter details
- `POST /api/adapters/:id/start` - Start an adapter
- `POST /api/adapters/:id/stop` - Stop an adapter

### Heartbeat & NodeMap
- `GET /api/heartbeat/stats` - Heartbeat statistics
- `GET /api/heartbeat/nodes` - NodeMap entries

### Failover
- `GET /api/failover/events` - Recent failover events
- `POST /api/failover/force` - Force failover to specific adapter

### Configuration
- `GET /api/config/network` - Get network configuration
- `POST /api/config/network` - Update network configuration

## Customization

### Theming

Colors are defined in CSS custom properties in `+layout.svelte`. The default theme is dark with these colors:

- Background: `#111827` (gray-900)
- Card background: `#1f2937` (gray-800)
- Borders: `#374151` (gray-700)
- Primary accent: `#3b82f6` (blue-500)
- Success: `#10b981` (green-500)
- Warning: `#f59e0b` (amber-500)
- Error: `#ef4444` (red-500)

### Adding New Pages

1. Create a new directory in `src/routes/`
2. Add a `+page.svelte` file
3. Add navigation link in `src/routes/+layout.svelte`

Example:

```svelte
<!-- src/routes/messages/+page.svelte -->
<script lang="ts">
  import { messageQueue } from '$lib/stores';
</script>

<h1>Messages</h1>
<!-- Your content here -->
```

## Privacy & Security

- **No external dependencies in production** - All assets are bundled
- **No telemetry or tracking** - No data sent to third parties
- **Local-only communication** - Dashboard only talks to local MyriadNode instance
- **No cookies** - State managed in memory with Svelte stores
- **Open source** - Fully auditable code

## Technology Stack

- **Svelte 4** - Reactive UI framework (compiles to vanilla JS)
- **SvelteKit 2** - Application framework with routing
- **TypeScript** - Type safety
- **Vite** - Build tool and dev server
- **Chart.js** - Charting library (optional, for future visualizations)

## Performance

- **Bundle size**: ~3KB (Svelte runtime)
- **Total size**: ~50KB (including all code and styles)
- **Load time**: < 100ms on local network
- **Memory usage**: < 10MB

Compare to alternatives:
- React: ~45KB runtime
- Angular: ~100KB+ runtime
- Vue: ~35KB runtime

## License

Same as MyriadMesh project (check root LICENSE file)

## Contributing

1. Follow the existing code style
2. Test all changes in development mode
3. Build production bundle to verify no errors
4. Update this README if adding new features

## Troubleshooting

### "API error: 404"

The MyriadNode backend is not running or not accessible. Start it with:

```bash
cargo run --package myriadnode
```

### "Connection refused"

Check that:
1. MyriadNode is running on port 8080
2. No firewall blocking the connection
3. API endpoint in `vite.config.js` is correct

### Build errors

Clear the cache and reinstall:

```bash
rm -rf node_modules .svelte-kit
npm install
npm run build
```

## Future Enhancements

- [ ] Real-time updates via WebSocket
- [ ] Network topology graph visualization
- [ ] Message queue monitoring
- [ ] Performance charts and history
- [ ] Mobile-responsive design improvements
- [ ] Dark/light theme toggle
- [ ] Export data to CSV/JSON
