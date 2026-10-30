window.addUploadTagPair = (relationship, tag) => {
  const normalizedRelationship = String(relationship).trim().toLowerCase().normalize('NFC');
  const normalizedTag = String(tag).trim().toLowerCase().normalize('NFC');
  const message = document.getElementById('upload-tag-error');
  if (!normalizedRelationship || !normalizedTag) {
    message.textContent = 'Enter both a relationship and a tag.';
    return;
  }
  message.textContent = '';

  const selectedTags = document.getElementById('selected-upload-tags');
  document.getElementById('upload-tag-empty')?.remove();
  const duplicate = [...selectedTags.querySelectorAll('input[name="tag_pair"]')].some((input) => {
    const pair = new URLSearchParams(input.value);
    return pair.get('relationship') === normalizedRelationship && pair.get('tag') === normalizedTag;
  });
  if (duplicate) return;

  const pair = new URLSearchParams();
  pair.set('relationship', normalizedRelationship);
  pair.set('tag', normalizedTag);

  const chip = document.createElement('span');
  chip.className = 'selected-upload-tag';
  const input = document.createElement('input');
  input.type = 'hidden';
  input.name = 'tag_pair';
  input.value = pair.toString();
  chip.append(input);

  const label = document.createElement('span');
  label.className = 'selected-upload-tag-label';
  label.textContent = `#${normalizedTag} · ${normalizedRelationship}`;
  chip.append(label);

  const remove = document.createElement('button');
  remove.type = 'button';
  remove.className = 'selected-upload-tag-remove';
  remove.setAttribute('aria-label', `Remove ${normalizedTag} under ${normalizedRelationship}`);
  remove.title = remove.getAttribute('aria-label');
  remove.textContent = '×';
  remove.addEventListener('click', () => {
    chip.remove();
    if (!selectedTags.querySelector('input[name="tag_pair"]')) {
      const empty = document.createElement('p');
      empty.id = 'upload-tag-empty';
      empty.className = 'media-muted';
      empty.textContent = 'No additional tags selected.';
      selectedTags.append(empty);
    }
  });
  chip.append(remove);
  selectedTags.append(chip);
};

window.clearProcessedUploads = () => {
  const feed = document.getElementById('processed-feed');
  feed.replaceChildren();
  const placeholder = document.createElement('p');
  placeholder.id = 'feed-placeholder';
  placeholder.className = 'feed-placeholder';
  placeholder.textContent = 'No files uploaded yet. Select files above to begin.';
  feed.append(placeholder);
};
