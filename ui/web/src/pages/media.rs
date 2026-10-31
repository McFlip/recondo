use maud::{Markup, html};
use recondo_domain::model::{Media, Tag};
use recondo_domain::{CountedTag, MediaQueryItems, MediaQueryResult, RelationshipTagGroup};

use crate::layout;

pub const DEFAULT_LIMIT: usize = 10;
pub const LIMIT_OPTIONS: [usize; 8] = [10, 25, 50, 75, 100, 150, 200, 400];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Operation {
    #[default]
    And,
    Or,
    AndThenOr,
    OrThenAnd,
}

impl Operation {
    pub fn is_grouped(self) -> bool {
        matches!(self, Self::AndThenOr | Self::OrThenAnd)
    }

    fn query_value(self) -> &'static str {
        match self {
            Self::And => "and",
            Self::Or => "or",
            Self::AndThenOr => "and_then_or",
            Self::OrThenAnd => "or_then_and",
        }
    }

    pub fn single_group_operation(self) -> Self {
        match self {
            Self::OrThenAnd => Self::Or,
            _ => Self::And,
        }
    }

    fn group_separator(self) -> &'static str {
        match self {
            Self::OrThenAnd => "AND Any Of",
            _ => "OR All Of",
        }
    }
}

pub struct GroupState<'a> {
    pub groups: &'a [Vec<String>],
    pub add_to_groups: &'a [usize],
}

pub fn render(
    selected_tags: &[Tag],
    group_state: GroupState<'_>,
    limit: usize,
    operation: Operation,
    result: Option<&MediaQueryResult>,
    error: Option<&str>,
    empty_message: &str,
) -> Markup {
    let tag_ids = selected_tags
        .iter()
        .map(|tag| tag.id().to_string())
        .collect::<Vec<_>>();
    let groups = group_state.groups;
    let add_to_groups = group_state.add_to_groups;
    layout::render(
        "Media results",
        html! {
            section class="shell media-shell section-space" {
                div class="section-heading media-heading" {
                    p class="eyebrow" { "Follow the thread" }
                    h1 { "Your " em { "media." } }
                    p class="section-description" {
                        "Combine tags and discover the media that connects them."
                    }
                }

                div class="media-workspace" {
                    aside
                        class="media-sidebar"
                        aria-label=(if matches!(operation, Operation::Or) || operation.is_grouped() {
                            "Add tags"
                        } else {
                            "Related tags"
                        }) {
                        div class="media-sidebar-heading" {
                            p class="eyebrow" {
                                @if operation == Operation::Or {
                                    "BUILD YOUR OR SEARCH"
                                } @else if operation.is_grouped() {
                                    "BUILD YOUR GROUPS"
                                } @else {
                                    "DISCOVER"
                                }
                            }
                            h2 {
                                @if matches!(operation, Operation::Or) || operation.is_grouped() {
                                    "Add tags"
                                } @else {
                                    "Related tags"
                                }
                            }
                        }
                        @if matches!(operation, Operation::Or) || operation.is_grouped() {
                            (relationship_picker(
                                &tag_ids,
                                groups,
                                add_to_groups,
                                limit,
                                operation,
                            ))
                        } @else {
                            nav class="relationship-accordion-list" aria-label="Tags grouped by relationship" {
                                @if let Some(result) = result {
                                    @for group in &result.relationships {
                                        (relationship_group(group, &tag_ids, limit))
                                    }
                                }
                                @if result.is_none_or(|result| result.relationships.is_empty()) {
                                    p class="media-muted" {
                                        @if selected_tags.is_empty() {
                                            "Choose tags to see related connections."
                                        } @else {
                                            "No related tags were found."
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div class="media-main" {
                        nav class="media-toolbar" aria-label="Media search controls" {
                            section class="media-controls-section" aria-label="Search options" {
                                div class="operation-control" {
                                    p class="media-control-label" { "LOGICAL OPERATION" }
                                    div class="operation-buttons" {
                                        (operation_button(
                                            "AND",
                                            Operation::And,
                                            operation,
                                            &tag_ids,
                                            limit,
                                        ))
                                        (operation_button(
                                            "OR",
                                            Operation::Or,
                                            operation,
                                            &tag_ids,
                                            limit,
                                        ))
                                        (group_mode_button(
                                            "AND then OR",
                                            Operation::AndThenOr,
                                            &tag_ids,
                                            groups,
                                            add_to_groups,
                                            operation,
                                            limit,
                                        ))
                                        (group_mode_button(
                                            "OR then AND",
                                            Operation::OrThenAnd,
                                            &tag_ids,
                                            groups,
                                            add_to_groups,
                                            operation,
                                            limit,
                                        ))
                                    }
                                }
                                form class="limit-control" method="get" action="/media" {
                                    (hidden_search_state(&tag_ids, groups, add_to_groups, operation))
                                    label for="display-limit" class="media-control-label" { "DISPLAY LIMIT" }
                                    select
                                        id="display-limit"
                                        name="limit"
                                        data-on:change="el.form.requestSubmit()" {
                                        option
                                            value=(DEFAULT_LIMIT)
                                            selected[limit == DEFAULT_LIMIT] {
                                            (DEFAULT_LIMIT)
                                        }
                                        @for option in LIMIT_OPTIONS.into_iter().skip(1) {
                                            option value=(option) selected[limit == option] { (option) }
                                        }
                                        option value="unlimited" selected[limit == usize::MAX] { "Unlimited" }
                                    }
                                    p class="media-control-note" {
                                        "Media is shown only when the number of results is under this limit."
                                    }
                                }
                            }

                            section class="selected-tags-section" aria-label="Selected tags" {
                                p class="media-control-label" { "SELECTED TAGS" }
                                @if operation.is_grouped() {
                                    (group_editor(
                                        selected_tags,
                                        groups,
                                        add_to_groups,
                                        limit,
                                        operation,
                                    ))
                                } @else if selected_tags.is_empty() {
                                    p class="media-muted" { "No tags selected" }
                                } @else {
                                    div class="selected-tag-list" {
                                        @for tag in selected_tags {
                                            (selected_tag(tag, &tag_ids, limit, operation))
                                        }
                                    }
                                }
                            }
                        }

                        section class="media-results" aria-live="polite" {
                            @if let Some(error) = error {
                                p class="media-error" { "Media search failed: " (error) }
                            } @else if let Some(result) = result {
                                (query_results(&result.media, operation))
                            } @else {
                                p class="media-empty-state" { (empty_message) }
                            }
                        }
                    }
                }
            }

            dialog
                id="media-lightbox"
                class="media-lightbox"
                aria-label="Full-size image"
                data-on:click="if (evt.target === el) el.close()" {
                button
                    type="button"
                    class="media-lightbox-close"
                    aria-label="Close image"
                    data-on:click="el.closest('dialog').close()" {
                    "×"
                }
                button
                    type="button"
                    id="media-lightbox-prev"
                    class="media-lightbox-nav media-lightbox-prev"
                    aria-label="Previous image"
                    data-on:click="window.mediaLightboxPrev()" {
                    "‹"
                }
                button
                    type="button"
                    id="media-lightbox-next"
                    class="media-lightbox-nav media-lightbox-next"
                    aria-label="Next image"
                    data-on:click="window.mediaLightboxNext()" {
                    "›"
                }
                img id="media-lightbox-image" alt="Full-size media preview";
                a id="media-lightbox-open" class="media-lightbox-open" aria-label="Open details" href="#" { "Open" }
            }
            script src="/media.js" {}
        },
    )
}

fn relationship_group(
    group: &RelationshipTagGroup,
    selected_ids: &[String],
    limit: usize,
) -> Markup {
    html! {
        details class="relationship-accordion" {
            summary {
                span { (group.relationship) }
                span class="relationship-accordion-count" { (group.tags.len()) }
            }
            div class="relationship-accordion-tags" {
                @for counted_tag in &group.tags {
                    (related_tag(counted_tag, selected_ids, limit))
                }
            }
        }
    }
}

fn related_tag(counted_tag: &CountedTag, selected_ids: &[String], limit: usize) -> Markup {
    let href = media_url(
        &selected_ids
            .iter()
            .cloned()
            .chain(std::iter::once(counted_tag.tag.id().to_string()))
            .collect::<Vec<_>>(),
        limit,
        Operation::And,
    );
    html! {
        a class="related-tag-link" href=(href) {
            span { (counted_tag.tag.name()) }
            span class="related-tag-count" { (counted_tag.count) }
        }
    }
}

fn group_editor(
    selected_tags: &[Tag],
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    limit: usize,
    operation: Operation,
) -> Markup {
    html! {
        div class="media-groups" {
            @for (index, group) in groups.iter().enumerate() {
                @if index > 0 {
                    p class="media-group-operator" { (operation.group_separator()) }
                }
                section class="media-tag-group" aria-label=(format!("Group {}", index + 1)) {
                    div class="media-tag-group-heading" {
                        form
                            class="media-group-target-form"
                            method="get"
                            action="/media" {
                            input type="hidden" name="operation" value=(operation.query_value());
                            input type="hidden" name="limit" value=(limit_value(limit));
                            @for group in groups {
                                input type="hidden" name="group" value=(group.join(","));
                            }
                            @for target in add_to_groups {
                                @if *target != index {
                                    input type="hidden" name="target" value=(target);
                                }
                            }
                            label class="media-group-target-label" {
                                input
                                    type="checkbox"
                                    name="target"
                                    value=(index)
                                    checked[add_to_groups.contains(&index)]
                                    data-on:change="el.form.requestSubmit()";
                                span { "Add new tags to this group" }
                            }
                        }
                        a
                            class="media-group-delete"
                            href=(delete_group_url(groups, add_to_groups, index, limit, operation))
                            aria-label=(format!("Delete group {}", index + 1))
                            title="Delete group" {
                            svg class="media-trash-icon" viewBox="0 0 24 24" aria-hidden="true" {
                                path d="M4 7h16M10 11v6m4-6v6M5 7l1 14h12l1-14M9 7V4h6v3";
                            }
                        }
                    }
                    @if group.is_empty() {
                        p class="media-muted media-group-empty" { "No tags in this group yet." }
                    } @else {
                        div class="selected-tag-list" {
                            @for id in group {
                                @if let Some(tag) = selected_tags.iter().find(|tag| tag.id() == id) {
                                    (grouped_selected_tag(
                                        tag,
                                        groups,
                                        add_to_groups,
                                        index,
                                        limit,
                                        operation,
                                    ))
                                }
                            }
                        }
                    }
                }
            }
            (add_group_button(groups, add_to_groups, limit, operation))
        }
    }
}

fn grouped_selected_tag(
    tag: &Tag,
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    group_index: usize,
    limit: usize,
    operation: Operation,
) -> Markup {
    let mut remaining_groups = groups.to_vec();
    remaining_groups[group_index].retain(|id| id != tag.id());
    let has_tags = remaining_groups.iter().any(|group| !group.is_empty());
    let href = if has_tags {
        group_state_url(&remaining_groups, add_to_groups, limit, operation)
    } else {
        "/explore".to_string()
    };
    html! {
        span class="selected-tag-chip" data-tag-id=(tag.id()) {
            span { (tag.name()) }
            a
                class="selected-tag-remove"
                href=(href)
                aria-label=(format!("Remove {} from group {}", tag.name(), group_index + 1))
                title=(format!("Remove {}", tag.name())) {
                "×"
            }
        }
    }
}

fn selected_tag(tag: &Tag, selected_ids: &[String], limit: usize, operation: Operation) -> Markup {
    let remaining = selected_ids
        .iter()
        .filter(|id| id.as_str() != tag.id())
        .cloned()
        .collect::<Vec<_>>();
    let href = if remaining.is_empty() {
        "/explore".to_string()
    } else {
        media_url(&remaining, limit, operation)
    };
    html! {
        span class="selected-tag-chip" {
            span { (tag.name()) }
            a
                class="selected-tag-remove"
                href=(href)
                aria-label=(format!("Remove {}", tag.name()))
                title=(format!("Remove {}", tag.name())) {
                "×"
            }
        }
    }
}

fn group_state_url(
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    limit: usize,
    operation: Operation,
) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("operation", operation.query_value());
    for group in groups {
        serializer.append_pair("group", &group.join(","));
    }
    for target in add_to_groups {
        serializer.append_pair("target", &target.to_string());
    }
    serializer.append_pair("limit", &limit_value(limit));
    format!("/media?{}", serializer.finish())
}

fn add_group_button(
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    limit: usize,
    operation: Operation,
) -> Markup {
    let mut groups = groups.to_vec();
    groups.push(Vec::new());
    let mut add_to_groups = add_to_groups.to_vec();
    add_to_groups.push(groups.len() - 1);
    html! {
        form class="media-add-group-form" method="get" action="/media" {
            (hidden_search_state(&[], &groups, &add_to_groups, operation))
            input type="hidden" name="limit" value=(limit_value(limit));
            button
                type="submit"
                class="media-add-group operation-button"
                aria-label="Add group" {
                "+ Add group"
            }
        }
    }
}

fn delete_group_url(
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    index: usize,
    limit: usize,
    operation: Operation,
) -> String {
    let mut remaining_groups = groups.to_vec();
    remaining_groups.remove(index);
    if remaining_groups.len() <= 1 {
        let first_group = remaining_groups.first().cloned().unwrap_or_default();
        if first_group.is_empty() {
            return "/explore".to_string();
        }
        return media_url(&first_group, limit, operation.single_group_operation());
    }
    let remaining_targets = add_to_groups
        .iter()
        .filter_map(|target| match target.cmp(&index) {
            std::cmp::Ordering::Less => Some(*target),
            std::cmp::Ordering::Greater => Some(target - 1),
            std::cmp::Ordering::Equal => None,
        })
        .collect::<Vec<_>>();
    group_state_url(&remaining_groups, &remaining_targets, limit, operation)
}

fn media_url(tag_ids: &[String], limit: usize, operation: Operation) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for tag_id in tag_ids {
        serializer.append_pair("tag", tag_id);
    }
    serializer.append_pair("limit", &limit_value(limit));
    match operation {
        Operation::Or => {
            serializer.append_pair("operation", "or");
        }
        Operation::AndThenOr => {
            serializer.append_pair("operation", "and_then_or");
        }
        Operation::OrThenAnd => {
            serializer.append_pair("operation", "or_then_and");
        }
        Operation::And => {}
    }
    format!("/media?{}", serializer.finish())
}

fn hidden_search_state(
    tag_ids: &[String],
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    operation: Operation,
) -> Markup {
    html! {
        @if operation.is_grouped() {
            input type="hidden" name="operation" value=(operation.query_value());
            @for group in groups {
                input type="hidden" name="group" value=(group.join(","));
            }
            @for target in add_to_groups {
                input type="hidden" name="target" value=(target);
            }
        } @else {
            @for id in tag_ids {
                input type="hidden" name="tag" value=(id);
            }
            @if operation == Operation::Or {
                input type="hidden" name="operation" value="or";
            }
        }
    }
}

fn operation_button(
    label: &str,
    target: Operation,
    current: Operation,
    tag_ids: &[String],
    limit: usize,
) -> Markup {
    let active = target == current;
    html! {
        form class="operation-form" method="get" action="/media" {
            @for id in tag_ids {
                input type="hidden" name="tag" value=(id);
            }
            input type="hidden" name="limit" value=(limit_value(limit));
            @if target == Operation::Or {
                input type="hidden" name="operation" value="or";
            }
            button
                type="submit"
                class=(if active { "operation-button is-selected" } else { "operation-button" })
                aria-pressed=(active) {
                (label)
            }
        }
    }
}

fn group_mode_button(
    label: &str,
    target: Operation,
    tag_ids: &[String],
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    current: Operation,
    limit: usize,
) -> Markup {
    let groups = if current == target {
        groups.to_vec()
    } else {
        vec![tag_ids.to_vec(), Vec::new()]
    };
    let targets = if current == target {
        add_to_groups.to_vec()
    } else {
        vec![0]
    };
    let active = current == target;
    html! {
        form class="operation-form" method="get" action="/media" {
            input type="hidden" name="operation" value=(target.query_value());
            input type="hidden" name="limit" value=(limit_value(limit));
            @for group in &groups {
                input type="hidden" name="group" value=(group.join(","));
            }
            @for target in &targets {
                input type="hidden" name="target" value=(target);
            }
            button
                type="submit"
                class=(if active { "operation-button is-selected" } else { "operation-button" })
                aria-pressed=(active) {
                (label)
            }
        }
    }
}

fn query_results(items: &MediaQueryItems, operation: Operation) -> Markup {
    match items {
        MediaQueryItems::Count(count) => html! {
            div class="media-count-result" {
                span class="media-count-number" { (count) }
                p { "matching media items" }
                p class="media-control-note" { "Lower the display limit to show individual items." }
            }
        },
        MediaQueryItems::Items(items) if items.is_empty() => html! {
            p class="media-empty-state" {
                @if operation == Operation::Or {
                    "No media matches any selected tags."
                } @else if operation == Operation::OrThenAnd {
                    "No media matches all selected groups."
                } @else if operation.is_grouped() {
                    "No media matches any of the selected groups."
                } @else {
                    "No media matches all selected tags."
                }
            }
        },
        MediaQueryItems::Items(items) => html! {
            p class="media-result-summary" {
                "Showing " (items.len()) " " (if items.len() == 1 { "item" } else { "items" })
            }
            div class="media-item-grid" {
                @for item in items {
                    (media_card(item))
                }
            }
        },
    }
}

fn relationship_picker(
    selected_ids: &[String],
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    limit: usize,
    operation: Operation,
) -> Markup {
    let state = picker_state_query(selected_ids, groups, add_to_groups, limit, operation);
    html! {
        form
            id="media-relationship-search"
            class="relationship-search media-relationship-search"
            data-signals="{ mediaquery: '' }"
            data-state=(state)
            data-on:submit__prevent="@get('/media/relationship-submit?query='+encodeURIComponent($mediaquery)+'&'+el.dataset.state)" {
            label for="media-relationship-query" class="explore-search-label" {
                "SEARCH RELATIONSHIPS"
            }
            input
                id="media-relationship-query"
                type="search"
                name="query"
                placeholder="Try “place” or “to do”"
                autocomplete="off"
                aria-controls="media-relationship-suggestions"
                data-state=(state)
                data-bind:mediaquery=""
                data-on:input="@get('/media/relationship-search?prefix='+encodeURIComponent($mediaquery)+'&'+el.dataset.state)";
        }

        div
            id="media-relationship-suggestions"
            class="relationship-suggestions media-picker-suggestions"
            role="status"
            aria-live="polite" {
            p class="explore-hint" { "Type to search relationships, then choose a tag to add it." }
        }

        div id="media-relationship-details" class="media-picker-details" aria-live="polite" {
            p class="explore-hint" { "Choose a relationship to see available tags." }
        }
    }
}

pub fn picker_suggestions(
    prefix: &str,
    relationships: &[String],
    selected_ids: &[String],
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    limit: usize,
    operation: Operation,
) -> Markup {
    let state = picker_state_query(selected_ids, groups, add_to_groups, limit, operation);
    html! {
        div id="media-relationship-suggestions" class="relationship-suggestions media-picker-suggestions" role="status" aria-live="polite" {
            @if prefix.trim().is_empty() {
                p class="explore-hint" { "Type to search relationships, then choose a tag to add it." }
            } @else if relationships.is_empty() {
                p class="explore-hint" { "No relationships match “" (prefix) "”." }
            } @else {
                p class="suggestion-count" {
                    "RELATIONSHIPS"
                    span { (relationships.len()) }
                }
                ul class="relationship-option-list" {
                    @for relationship in relationships {
                        li {
                            button
                                type="button"
                                class="relationship-option"
                                data-relationship=(relationship)
                                data-state=(state)
                                data-on:click="@get('/media/relationship?name='+encodeURIComponent(el.dataset.relationship)+'&'+el.dataset.state)" {
                                span { (relationship) }
                                span class="relationship-option-arrow" aria-hidden="true" { "↗" }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn picker_relationship_tags(
    relationship: &str,
    tags: &[Tag],
    selected_ids: &[String],
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    limit: usize,
    operation: Operation,
) -> Markup {
    let available = tags
        .iter()
        .filter(|tag| !selected_ids.iter().any(|id| id == tag.id()))
        .collect::<Vec<_>>();
    html! {
        div id="media-relationship-details" class="media-picker-details" aria-live="polite" {
            article class="relationship-result-card media-picker-result-card" {
                div class="relationship-result-heading" {
                    div {
                        p class="eyebrow" { "RELATIONSHIP" }
                        h2 { (relationship) }
                    }
                    span class="relationship-tag-count" { (available.len()) " available" }
                }
                @if available.is_empty() {
                    p class="explore-hint" { "No unselected tags are available in this relationship." }
                } @else if operation.is_grouped() && add_to_groups.is_empty() {
                    p class="explore-hint" { "Choose one or more groups to add tags to." }
                } @else {
                    div class="relationship-tag-grid" {
                        @for tag in available {
                            a
                                class="relationship-tag-card media-picker-tag"
                                data-testid="or-picker-tag"
                                href=(if operation.is_grouped() {
                                    add_tag_to_groups_url(
                                        groups,
                                        add_to_groups,
                                        tag.id(),
                                        limit,
                                        operation,
                                    )
                                } else {
                                    media_url(
                                        &selected_ids
                                            .iter()
                                            .cloned()
                                            .chain(std::iter::once(tag.id().to_string()))
                                            .collect::<Vec<_>>(),
                                        limit,
                                        Operation::Or,
                                    )
                                }) {
                                span class="relationship-tag-mark" aria-hidden="true" { "+" }
                                strong { (tag.name()) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn picker_state_query(
    selected_ids: &[String],
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    limit: usize,
    operation: Operation,
) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("limit", &limit_value(limit));
    match operation {
        Operation::AndThenOr | Operation::OrThenAnd => {
            serializer.append_pair("operation", operation.query_value());
            for group in groups {
                serializer.append_pair("group", &group.join(","));
            }
            for target in add_to_groups {
                serializer.append_pair("target", &target.to_string());
            }
        }
        _ => {
            serializer.append_pair("selected", &selected_ids.join(","));
        }
    }
    serializer.finish()
}

fn add_tag_to_groups_url(
    groups: &[Vec<String>],
    add_to_groups: &[usize],
    tag_id: &str,
    limit: usize,
    operation: Operation,
) -> String {
    let mut updated_groups = groups.to_vec();
    for index in add_to_groups {
        if let Some(group) = updated_groups.get_mut(*index)
            && !group.iter().any(|id| id == tag_id)
        {
            group.push(tag_id.to_string());
        }
    }
    group_state_url(&updated_groups, add_to_groups, limit, operation)
}

fn limit_value(limit: usize) -> String {
    if limit == usize::MAX {
        "unlimited".to_string()
    } else {
        limit.to_string()
    }
}

pub fn picker_suggestion_error(message: &str) -> Markup {
    html! {
        div id="media-relationship-suggestions" class="relationship-suggestions media-picker-suggestions" role="status" aria-live="polite" {
            p class="explore-error" { (message) }
        }
    }
}

pub fn picker_relationship_error(message: &str) -> Markup {
    html! {
        div id="media-relationship-details" class="media-picker-details" aria-live="polite" {
            p class="explore-error" { (message) }
        }
    }
}

pub fn picker_message(message: &str) -> Markup {
    html! {
        div id="media-relationship-details" class="media-picker-details" aria-live="polite" {
            p class="explore-hint" { (message) }
        }
    }
}

pub fn render_details(media: &Media, tags: &[Tag]) -> Markup {
    let asset_url = format!("/asset/{}", media.id.as_str());
    let title = media.title.as_deref().unwrap_or_else(|| media.id.as_str());
    let mime_type = media
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");
    let size_display = match media.file_size {
        Some(s) if s < 1024 => format!("{s} B"),
        Some(s) if s < 1024 * 1024 => format!("{:.1} KB", s as f64 / 1024.0),
        Some(s) => format!("{:.2} MB", s as f64 / (1024.0 * 1024.0)),
        None => "Unknown".to_string(),
    };
    let download_name = media
        .url
        .to_file_path()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| media.id.as_str().to_string());

    layout::render(
        "Media details",
        html! {
            section class="shell media-details section-space" {
                button class="back-button" data-on:click="window.history.back()" { "← Back" }
                div class="details-meta" {
                    h1 { (title) }
                    p { "MIME: " (mime_type) }
                    p { "Size: " (size_display) }
                    p { "Views: " (media.view_count) }
                    p { "URL: " (media.url.as_str()) }
                    div class="tag-list" {
                        @for tag in tags {
                            span class="tag-chip" { (tag.name()) }
                        }
                    }
                }
                div class="details-media" {
                    @if mime_type.starts_with("image/") {
                        img class="details-image" src=(asset_url) alt=(title) data-on:click="el.classList.toggle('full-size')" {}
                    } @else if mime_type.starts_with("video/") {
                        video controls preload="metadata" src=(asset_url) { "Your browser does not support embedded video." }
                    } @else {
                        a class="media-download" href=(asset_url) download=(download_name) {
                            span class="download-icon" aria-hidden="true" { "↓" }
                            span { "Download file" }
                        }
                    }
                }
            }
            script src="/media.js" {}
        },
    )
}

fn media_card(item: &Media) -> Markup {
    let asset_url = format!("/asset/{}", item.id.as_str());
    let title = item.title.as_deref().unwrap_or_else(|| item.id.as_str());
    let download_name = item
        .url
        .to_file_path()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| item.id.as_str().to_string());
    let mime_type = item
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");

    html! {
        article class="media-item-card" data-testid="media-card" {
            div class="media-preview" {
                @if mime_type.starts_with("image/") {
                    img
                        src=(asset_url)
                        alt=(title)
                        loading="lazy"
                        data-media-id=(item.id.as_str())
                        data-on:click="window.openMediaLightbox(el)";
                } @else if mime_type.starts_with("video/") {
                    video controls preload="metadata" src=(asset_url) {
                        "Your browser does not support embedded video."
                    }
                } @else {
                    a class="media-download" href=(asset_url) download=(download_name) {
                        span class="download-icon" aria-hidden="true" { "↓" }
                        span { "Download file" }
                    }
                }
            }
            div class="media-item-info" {
                h3 { (title) }
                p { (mime_type) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use recondo_domain::model::{MediaId, MediaOpts};

    #[test]
    fn video_media_uses_native_player_controls() {
        let media = Media::new(
            MediaId::from_hash("video-id").unwrap(),
            url::Url::parse("file:///assets/video.mp4").unwrap(),
            Some(MediaOpts {
                title: Some("video.mp4".into()),
                mime_type: Some("video/mp4".into()),
                file_size: Some(100),
                view_count: None,
            }),
        );

        let html = media_card(&media).into_string();

        assert!(html.contains("<video controls"));
        assert!(html.contains("src=\"/asset/video-id\""));
        assert!(!html.contains("Download file"));
    }
}
