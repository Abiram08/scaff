# scaff corpus

This directory contains the **seed corpus** — the curated knowledge base that `scaff` searches when answering research questions about the Harness platform.

## Format

Each line in `seed.jsonl` is a single chunk in JSON form:

```json
{"id":"...","url":"...","title":"...","section":"...","content":"...","source":"seed"}
```

| field    | required | notes                                             |
|----------|----------|---------------------------------------------------|
| id       | yes      | unique stable id (kebab-case recommended)         |
| url      | yes      | source URL — appears in citations                 |
| title    | yes      | short page title                                  |
| section  | no       | the heading this chunk came from                  |
| content  | yes      | the text the LLM will read                        |
| source   | no       | provenance label, defaults to `"seed"`            |

Lines starting with `#` and blank lines are ignored.

## How scaff uses it

On first run, `scaff` looks for a seed in this order:

1. `$SCAFF_CORPUS_DIR/seed.jsonl` (if `SCAFF_CORPUS_DIR` is set)
2. `corpus/seed.jsonl` next to the `scaff` binary
3. `corpus/seed.jsonl` in the current working directory

If none of those exist, `scaff corpus update` downloads a fresh seed from the configured `DEFAULT_SEED_URL`.

On first research query, the seed is loaded into a local SQLite database at `~/.scaff/corpus.db` and a BM25 index is built in memory.

## Refreshing the corpus

```bash
# Download the latest remote seed
scaff corpus update

# Add chunks from a local JSONL file
scaff corpus add path/to/local.jsonl

# Crawl a single URL and add it as one or more chunks
scaff corpus crawl https://developer.harness.io/docs/some-page

# Wipe and start over
scaff corpus clear && scaff corpus update
```

## Contributing a chunk

To add a new entry to the seed, append a line to `seed.jsonl` in the same format. Keep content focused (200–800 words), specific, and grounded in the actual platform. Avoid marketing language — write as if documenting for a new engineer.

After adding, run:

```bash
scaff corpus add corpus/seed.jsonl
```

to re-ingest locally, or open a PR to ship the change in the next release.
