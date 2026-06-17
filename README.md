# Novum

Novum is a desktop research IDE for long-running scientific work. The first
version focuses on a local-first paper workbench: PDF preview, PaperQA-backed
question answering, a science skill market, and a dense Warp-inspired tool UI.

## Development

The desktop app lives in `apps/desktop` and uses Tauri, React, TypeScript, and
Vite.

```sh
cd apps/desktop
npm install
npm run dev
```

Run the native desktop shell:

```sh
cd apps/desktop
npm run tauri:dev
```

Build the frontend:

```sh
cd apps/desktop
npm run build
```

See `spec.md` for the product and architecture specification.
