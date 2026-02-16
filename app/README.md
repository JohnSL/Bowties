# Tauri + SvelteKit + TypeScript

This template should help get you started developing with Tauri, SvelteKit and TypeScript in Vite.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## Prerequisites

- [Node.js](https://nodejs.org/) (v18 or later recommended)
- [Rust](https://www.rust-lang.org/tools/install)
- Platform-specific dependencies for Tauri (see [Tauri Prerequisites](https://tauri.app/v1/guides/getting-started/prerequisites))

## Running the App

### Install Dependencies

```bash
npm install
```

### Development Mode

Run the app in development mode with hot-reload:

```bash
npm run tauri dev
```

This will:
1. Start the SvelteKit development server
2. Build the Rust backend
3. Launch the desktop application

### Type Checking

Run Svelte type checking:

```bash
npm run check
```

Or run in watch mode:

```bash
npm run check:watch
```

### Building for Production

Create a production build:

```bash
npm run tauri build
```

The compiled application will be available in `src-tauri/target/release/`.
