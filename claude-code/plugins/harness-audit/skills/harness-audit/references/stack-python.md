# Python Stack

Covers: Python 3.10+, FastAPI / Django / Flask, ML projects, data engineering, CLI tools.

## Tooling matrix

| Concern | Recommended | Alternatives |
|---------|-------------|--------------|
| Lint + format | ruff (single tool, very fast) | flake8 + black, pylint |
| Type checker | mypy or pyright | pyre |
| Test runner | pytest | unittest (stdlib), nose2 |
| Pre-commit | pre-commit framework (`pre-commit-config.yaml`) | Husky + lint-staged (works) |
| Package manager | uv (fast, recommended for new projects) | poetry, pip + pip-tools, pdm, hatch |

## Lint config paths to check

- `pyproject.toml` → `[tool.ruff]` section is the modern standard
- `ruff.toml`, `.ruff.toml`
- `setup.cfg` → `[flake8]` (legacy)
- `pyproject.toml` → `[tool.black]`, `[tool.isort]`, `[tool.mypy]`
- `.flake8`, `.pylintrc`

## Pre-commit pattern (using pre-commit framework)

`.pre-commit-config.yaml`:
```yaml
repos:
  - repo: https://github.com/astral-sh/ruff-pre-commit
    rev: v0.6.9
    hooks:
      - id: ruff
        args: [--fix]
      - id: ruff-format
  - repo: https://github.com/pre-commit/mirrors-mypy
    rev: v1.11.2
    hooks:
      - id: mypy
        additional_dependencies: [types-requests]  # add your stubs
```

Install: `pip install pre-commit && pre-commit install`

## Test wrapper

Usually no wrapper needed — `pytest` is the one-liner. Gaps to look for:

```toml
# pyproject.toml
[tool.pytest.ini_options]
testpaths = ["tests"]
addopts = "-ra -q --strict-markers"
```

If the project has slow integration tests, group them with markers:
```python
@pytest.mark.integration
def test_db_round_trip():
    ...
```

And expose `make test` / `make test-fast` / `make test-integration` so agents can run the right subset.

## CI pattern (GitHub Actions)

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        python-version: ['3.11', '3.12']
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: ${{ matrix.python-version }}
      - run: pip install -e '.[dev]'
      - run: ruff check .
      - run: ruff format --check .
      - run: mypy .
      - run: pytest
```

For uv-based projects, replace `pip install` with `uv sync --all-extras`.

## Python-specific gotchas to flag

- **Virtualenv assumptions** — agent runs `python -m pytest` outside venv, gets wrong interpreter. Document the activation in AGENTS.md.
- **`requirements.txt` vs `pyproject.toml` drift** — both present, only one is authoritative. Flag as a gap.
- **No type hints** — Python is gradually typed. A codebase with zero type hints leaves agents without static contracts. Recommend adding `mypy` even if loosely configured.
- **`__init__.py` vs implicit namespace packages** — modern Python prefers no `__init__.py`, but mixing is a footgun. Pick one convention.
- **`pytest` discovery** — if test files don't match `test_*.py` pattern, pytest won't find them. Audit the discovery config.
- **Mocked DB tests** — Lopopolo's "agents shouldn't mock the DB if migrations exist" applies hard here. Flag if integration tests use sqlite-in-memory but prod is Postgres.
- **`os.environ.get` without defaults** — silent None propagation. Recommend pydantic-settings or explicit fail-fast on missing env.

## Repo skills worth seeding

- `run-migration` — wraps alembic / django manage.py for the project
- `seed-test-data` — fixture loader
- `regen-types` — for projects using sqlacodegen, datamodel-code-generator, etc.

## Bonus signals (for the audit prompt)

- Python version pinned? (`python_requires`, `pyproject.toml#tool.poetry.dependencies.python`)
- Package manager (uv / poetry / pip + requirements.txt)
- ASGI vs WSGI framework
- Async-heavy or sync-heavy
- ML stack hints (torch, jax, tensorflow, transformers)
- Data eng hints (dagster, prefect, airflow, dbt)
- `Makefile` with conventional targets (test, lint, fmt, typecheck)
