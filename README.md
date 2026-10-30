# Recondo

**Discover more in your media collection.**

Recondo is a graph-based media discovery tool written in Rust. It connects media to descriptive tags, letting you explore collections through flexible searches rather than rigid folder structures or hierarchies.

Combine tags to narrow your search, follow connections to discover related media, and pivot between categories to explore your collection from different angles.

## Features

- **Tag-driven discovery** — Find media through descriptive tags and shared categories.
- **Flexible search** — Combine multiple tags using logical conditions to refine results.
- **Drill down and pivot** — Narrow your focus or explore a different dimension without losing the flexibility of a graph.
- **Relationship-based organization** — Connect media to multiple tags without imposing a fixed hierarchy.
- **Built with Rust** — Designed with performance and reliability in mind.

## Philosophy

Media collections shouldn't be limited by folders and rigid taxonomies. Recondo treats discovery as exploration: every tag offers another way to navigate your collection, uncover connections, and find things you might not have thought to search for.

In Recondo, tags can be combined using logical conditions to progressively narrow or broaden a search. Because tags are connected to media through a graph, searches can cross categories without following a fixed hierarchy.

For example, imagine searching a collection of images:

- AND — Find media tagged with both `landscape` and `sunset`.
- OR — Find media tagged with either `mountains` or `coast`.
- NOT — Exclude media tagged with `black-and-white`.

Users can then drill down by adding more conditions or pivot by changing the tags they're exploring. Since tags can belong to different categories, a search might combine a subject, a location, a style, and a date without requiring those categories to fit into a tree.

The key benefit is that each search becomes a flexible intersection of relationships, rather than a path through a predefined folder structure.

## Why Choose Recondo?

Traditional media managers organize content around folders, albums, or predefined taxonomies. These approaches work well when you already know where something belongs, but can make discovery difficult when you're exploring a large collection or don't know exactly what you're looking for.

Recondo takes a different approach. Rather than forcing media into a hierarchy, it connects media items to tags in a graph. You can combine tags across categories, narrow results with additional conditions, or pivot to a different dimension of your collection at any time.

This makes Recondo particularly useful when:

- You remember attributes, not locations. Find media by what it depicts, where it was taken, its style, or other descriptive tags.

- You want to discover, not just retrieve. Explore connections and uncover relevant items you might not have thought to search for.

- Your categories overlap. Combine independent dimensions without duplicating items across folders or maintaining a rigid taxonomy.

- Your collection doesn't fit a tree. Navigate relationships freely instead of committing to a single organizational structure.

Recondo is built around a simple idea: finding media should be an exploration, not a scavenger hunt through folders.

## Configuration

The web UI reads these environment variables at startup:

| Variable | Default | Description |
| --- | --- | --- |
| `RECONDO_DB_PATH` | `ui/web/db/graph.redb` | Path to the Recondo graph database. Parent directories are created on demand. |
| `RECONDO_ADDR` | `127.0.0.1:3000` | Address the web server binds to. |
| `RECONDO_MAX_UPLOAD_MB` | *(unset = unlimited)* | Maximum upload size in whole mebibytes. When unset, uploads are uncapped — they are streamed to temporary files, so memory use stays bounded regardless of size. Set this (e.g. `RECONDO_MAX_UPLOAD_MB=64`) to enforce a limit; an invalid value logs a warning and leaves uploads unlimited. |

## Status

Recondo is under development. Features, interfaces, and usage instructions will evolve as the project takes shape.

## The Name

*Recondo* is a portmanteau of **reconnaissance** and **commando**. The name became associated with the U.S. Army's long-range reconnaissance patrols during the Vietnam War, whose small teams ventured deep into enemy territory to observe, gather intelligence, and locate an elusive enemy. The MACV Recondo School was established in Vietnam in 1966 to train personnel for these demanding missions. <https://history.army.mil/portals/143/Images/Publications/catalog/90-21-1.pdf>

Recondo borrows that spirit of exploration: venture into a vast, unfamiliar collection, follow the leads, uncover connections, and find what you're looking for—even when you don't know exactly where to start.

## License

Recondo is licensed under the Apache License, Version 2.0.
