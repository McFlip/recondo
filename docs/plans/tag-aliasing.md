# Plan: Tag Aliasing

## Goal

Allow two tags to be aliased so their media references are mirrored. When a tag is
added to or removed from a media item, the same change is duplicated to every tag
aliased to it (same relationship). Aliasing should also copy existing references
in both directions at creation time.

## Concepts

- A **tag alias** is a symmetric, transitive relationship between tag IDs.
- Aliasing is stored in a dedicated alias table, not inferred from media edges.
- An alias group contains one or more tag IDs. All tags in a group share
  references.
- Propagation happens at the command/repository boundary so every entry point
  (upload, details page, bulk tag, recycle restore) benefits automatically.

## Data model changes

### Domain

- Add `AliasRepository` trait (or extend `TagRepository`) with:
  - `alias_tags(tag_a: &str, tag_b: &str) -> Result<(), E>`
  - `unalias_tags(tag_a: &str, tag_b: &str) -> Result<(), E>`
  - `get_aliases(tag_id: &str) -> Result<Option<Vec<String>>, E>`
  - `get_alias_group(tag_id: &str) -> Result<Option<Vec<String>>, E>`
- Add commands:
  - `alias_tags(source, dest, repo)`
  - `unalias_tags(source, dest, repo)`
- Possibly add `TagAlias` model for validation/normalization.

### DAL (redb)

- New table: `TAG_ALIASES: TableDefinition<&str, Vec<String>>` mapping a tag ID
  to its alias group members.
- `alias_tags`:
  - Normalize/validate both tag IDs exist (or allow future tags? Prefer require
    existing tag edges or at least tag names).
  - Merge alias groups if either tag already aliased.
  - Persist symmetric membership for all group members.
  - Copy references: for every edge of either tag, add the mirrored edge to the
    other tag (and relationship tag lists), without removing originals.
- `unalias_tags`:
  - Remove membership between the two tags.
  - If groups split, keep remaining alias relationships.
  - Do not remove already-copied media references (mirroring stops going
    forward; historical references remain).

## Propagation rules

- `add_tag_edge(media_id, tag, relationship)`:
  - After successfully adding the edge for `tag`, look up its alias group.
  - For each aliased tag ID, add the same `(relationship, media_id)` edge for
    that tag and ensure it appears in `RELATIONSHIP_TO_TAGS`.
  - Guard against re-entrancy/duplicate work by checking existing edges before
    inserting.
- `remove_tag_edge(media_id, tag_id, relationship)`:
  - After removing the edge for `tag_id`, look up aliases.
  - Remove the same edge for each aliased tag.
  - Update relationship tag lists only when no remaining edges use that
    relationship for that tag.
- Bulk operations (`add_tag_to_media_items`) call `add_tag_edge` per item, so
  propagation is inherited.
- Upload duplicate handling calls `apply_upload_tags_to_duplicate`, which calls
  `add_tag_edge` (or equivalent), so propagation is inherited.

## Command/API changes

- `add_tag_to_media` and `add_tag_to_media_items` remain unchanged signatures;
  propagation happens in repository.
- Add `alias_tags` and `unalias_tags` commands in `domain/src/cmd.rs`.
- Add query helpers in `domain/src/qry.rs` if needed (e.g. `get_aliases`).

## UI changes

- Explore/relationship management page: add alias management controls, similar to
  merge relationship page.
- Media details page: optionally show aliased tags.
- No changes required to upload flow; alias propagation is automatic.

## Migration/compatibility

- New table created lazily on first use; `init_db` should create it.
- Existing databases without the table must be handled gracefully (treat as
  no aliases).
- Tag existence checks should not break stub repositories in tests; update
  `TagRepository` trait and all implementations/stubs.

## Testing plan

- Domain unit tests for alias/unalias validation.
- DAL tests:
  - aliasing copies existing references both ways.
  - adding a tag to media mirrors to aliased tag.
  - removing a tag mirrors removal.
  - unalias stops future propagation but keeps prior references.
- Web/integration tests:
  - alias two tags, upload/tag media, verify both appear.
  - bulk tag applies to aliases.
- Playwright E2E:
  - alias tags via UI and verify mirrored search results.

## Risks

- Infinite recursion if propagation calls repository methods that re-check
  aliases. Implement propagation in a single transaction or with an
  `include_aliases: bool` internal flag.
- Partial failures during multi-tag propagation. Prefer one write transaction
  per operation.
- Relationship tag lists can become stale if alias group changes; rebuild or
  update incrementally.
- Removing a tag edge currently removes the tag from `RELATIONSHIP_TO_TAGS`
  only when that relationship is no longer used; alias propagation must
  respect the same rule per tag.
EOF
mkdir -p docs/plans && mv PLAN.md docs/plans/tag-aliasing.md && echo created