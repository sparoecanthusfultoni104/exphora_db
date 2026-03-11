# ExphoraDB

A fast, lightweight dataset viewer and explorer for your desktop.
Open your data, filter it, analyze it, share it — no cloud required.

![Version](https://img.shields.io/badge/version-v0.7.2-purple)
![Platform](https://img.shields.io/badge/platform-Windows_11_Pro-lightgrey)

---

## Tech Stack

Rust backend, React frontend, no nonsense.

- **Core & Backend** — [Tauri v2](https://v2.tauri.app/) + Rust
- **Frontend** — [React 19](https://react.dev/) + TypeScript
- **State** — [Zustand](https://github.com/pmndrs/zustand)
- **Styles** — [Tailwind CSS v4](https://tailwindcss.com/)
- **Charts** — [Recharts](https://recharts.org/)
- **Icons** — [Lucide React](https://lucide.dev/)

---

## Supported Formats

Open, explore and export all of these:

| Format | Extensions |
| :--- | :--- |
| Exphora Views | `.exh` |
| JSON | `.json` |
| JSON Lines / NDJSON | `.jsonl`, `.ndjson` |
| CSV | `.csv` |
| XML | `.xml` |
| SQLite | `.db`, `.sqlite`, `.sqlite3` |

Export to: CSV, JSON, Excel, Markdown, PDF.

*(**Pro tip**: the `.exh` format acts as a complete saved workspace for your session, seamlessly preserving active filters, column configurations, chart states, and dataset paths. It also features **floating markdown notes** per view, **column annotations**, Smart Save, and native OS file associations.)*

---

## Architecture

Two clear layers, nothing weird.

**`src/` — Rust backend**
Native OS integration via Tauri. Handles file I/O, schema inference
(`parser.rs`), filtering and stats (`filters.rs`), expression eval
(`expr.rs`) and the full P2P layer (`p2p/`).

**`ui/src/` — React frontend**
Modular SPA split into:

- `components/` — virtualized tables, sidebars, modals, overlays, charts
- `hooks/` — `useDataset`, `useFilters`, `useFocusTrap`, `useAutoSave` and more
- `store/` — global tab state via Zustand (`appStore.ts`)

---

## Keyboard Shortcuts

Full keyboard navigation. Mouse optional.

### Files & Tabs

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + O` | Open file picker |
| `Ctrl + R` | Reload active dataset |
| `Ctrl + W` | Close active tab |
| `Ctrl + Tab` | Next tab |
| `Ctrl + Shift + Tab` | Previous tab |
| `Ctrl + 1..9` | Jump to tab by number |

### Search & Navigation

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + F` | Focus global table search |
| `Tab` / `Shift+Tab` | Move between interactive elements (focus trapped inside overlays) |
| `Arrow keys` | Navigate tabs, menus and modal lists |
| `Enter` | Confirm selection or open column context menu |
| `Escape` | Close any active panel or modal |

### Table Actions

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + Shift + F` | Open column picker — Filter |
| `Ctrl + Shift + S` | Open column picker — Stats |
| `Ctrl + Shift + G` | Open column picker — Frequency chart |
| `Ctrl + Shift + C` | Clear all active filters instantly |
| `Ctrl + E` | Open export dialog |
| `Double click` | Edit cell inline |
| `Enter` | Confirm cell edit |
| `Escape` | Cancel cell edit |
| `Ctrl + Z` | Undo last edit |
| `Ctrl + X` | Redo last undone edit |

### App

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + D` | Toggle dark / light theme |
| `Ctrl + ,` | Open settings |
| `Ctrl + P` | Open P2P share panel |

---

## Running the Project

You need [Node.js](https://nodejs.org/) and [Rust](https://rustup.rs/) installed.

```bash
# Install frontend dependencies
cd ui && npm install

# Dev mode — hot reload React + Rust
cargo tauri dev

# Production build
npx @tauri-apps/cli build

```

## Test

```bash
cargo test
# 37 passed, 0 failed
```
