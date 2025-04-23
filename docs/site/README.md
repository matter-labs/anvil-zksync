# anvil-zksync Docs

This is the documentation site for `anvil-zksync`, built with [Vocs](https://github.com/wevm/vocs) —
a minimal, fast, and Markdown-first documentation framework powered by Vite.

## 📦 Project Structure

```bash
.
├── pages/          # Main docs content in .mdx (CLI, guides, RPC)
├── components/     # Custom React components for MDX
├── public/         # Static assets like logos and favicons
├── styles.css      # Global styles
├── vocs.config.ts  # Vocs site configuration
```

## 🛠️ Development

### Start local dev server

```bash
bun run dev
```

### Build static site

```bash
bun run build
```

### Preview built site

```bash
bun run preview
```

## 🧹 Lint & Format

### Format codebase

```bash
bun run format
```

### Lint and fix issues

```bash
bun run lint
```

## ✨ Notes

- Pages are authored using MDX under `./pages/`
- You can add custom components in `./components/` and import them into `.mdx` files
- Static assets live in `./public/` and are served as-is

## 🧪 Requirements

- [Bun](https://bun.sh/)
- Node ≥ 18
