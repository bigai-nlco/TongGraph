# Repository Guidelines

## Project Structure & Module Organization

TongGraph is a Rust core exposed to Python through PyO3. Rust source lives in
`src/`: `core.rs` owns graph behavior, `models.rs` defines records and Python
wrapper classes, `sqlite.rs` handles persistence, `codec.rs` handles encoding,
and `py_api.rs` registers the Python module. The Python package surface is in
`python/tonggraph/`. Python integration tests live in `tests/`. Build helpers
are in `scripts/`, especially `scripts/build_python_extension.py`. Generated
artifacts under `target/`, `.venv/`, caches, local `.db` files, and built
extension modules are ignored.

## Build, Test, and Development Commands

- `uv sync --dev`: create or update the Python development environment.
- `cargo test`: run Rust tests for the core crate.
- `uv run python scripts/build_python_extension.py`: build the PyO3 extension
  in place at `python/tonggraph/_tonggraph*.so`.
- `uv run python scripts/build_python_extension.py --release`: build an
  optimized local extension.
- `uv run pytest`: run the Python SDK tests in `tests/`.
- `cargo fmt`: format Rust code before submitting changes.

## Coding Style & Naming Conventions

Use Rust 2021 idioms and keep modules focused on their current responsibilities.
Rust functions and fields use `snake_case`; Rust types use `PascalCase`.
Prefer explicit `Result<_, String>` handling in the core and convert to Python
exceptions at the PyO3 boundary. Python tests and helpers use standard
`snake_case` names. Keep public Python API names stable and ergonomic, matching
current methods such as `add_node`, `add_edge`, `neighbors`, and `k_hop`.

## Testing Guidelines

Use pytest for Python-facing behavior. Name tests `test_<behavior>` and keep
fixtures local unless they are reused. Cover both in-memory graphs and
SQLite-backed reopening when persistence or indexing changes. Build the
extension before running Python tests. For API changes, add assertions for
return values, ordering, error handling, and persisted state where relevant.

## Commit & Pull Request Guidelines

This repository follows [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/) to create new commit messages. Use
short, imperative commit subjects, for example `add sqlite edge index` or
`fix k-hop direction filtering`. Keep commits scoped to one logical change.
Pull requests should describe the change, list commands run, note API or storage
format impacts, and link any relevant issue. Include screenshots only for
documentation or UI-related changes.

## Security & Configuration Tips

Do not commit local databases, virtual environments, caches, or built extension
artifacts. Treat SQLite files created during testing as disposable local data.
