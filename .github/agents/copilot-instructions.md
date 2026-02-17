# bowtie Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-14

## Active Technologies
- Rust 2021 edition (backend), TypeScript 5.6 (frontend) (001-node-snip-data)
- In-memory for this feature (future: SQLite for CDI cache) (001-node-snip-data)

- Python 3.12 (latest stable as of 2026), managed via UV + PySerial (serial port communication), IntelHex (firmware loading), UV (Python version management) (001-python3-migration)

## Project Structure

```text
src/
tests/
```

## Commands

cd src; pytest; ruff check .

## Code Style

Python 3.12 (latest stable as of 2026), managed via UV: Follow standard conventions

## Recent Changes
- 001-node-snip-data: Added Rust 2021 edition (backend), TypeScript 5.6 (frontend)

- 001-python3-migration: Added Python 3.12 (latest stable as of 2026), managed via UV + PySerial (serial port communication), IntelHex (firmware loading), UV (Python version management)

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
