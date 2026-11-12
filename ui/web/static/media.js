let currentLightboxIndex = -1;
let currentMediaId = null;

function mediaDetailsUrl(mediaId) {
  const params = new URLSearchParams({ id: mediaId });
  if (window.location.pathname === '/media') {
    params.set('return_to', window.location.pathname + window.location.search);
  }
  return `/media/details?${params}`;
}

function getImageItems() {
  return Array.from(document.querySelectorAll('.media-preview img'));
}

window.openMediaLightbox = (clickedImg) => {
  const images = getImageItems();
  const index = images.indexOf(clickedImg);
  if (index === -1) {
    currentLightboxIndex = 0;
  } else {
    currentLightboxIndex = index;
  }

  const dialog = document.getElementById('media-lightbox');
  if (!dialog) return;

  const scrollX = window.scrollX;
  const scrollY = window.scrollY;

  dialog.showModal();
  window.updateMediaLightbox();
  const lightboxImg = document.getElementById('media-lightbox-image');
  if (lightboxImg) {
    lightboxImg.decode?.().catch(() => {});
  }

  const closeBtn = dialog.querySelector('.media-lightbox-close');
  if (closeBtn) {
    closeBtn.focus({ preventScroll: true });
  }
  window.scrollTo(scrollX, scrollY);
  requestAnimationFrame(() => window.scrollTo(scrollX, scrollY));
};

window.updateMediaLightbox = () => {
  const images = getImageItems();
  const dialog = document.getElementById('media-lightbox');
  const lightboxImg = document.getElementById('media-lightbox-image');
  const prevBtn = document.getElementById('media-lightbox-prev');
  const nextBtn = document.getElementById('media-lightbox-next');

  if (!dialog || !lightboxImg || images.length === 0) return;

  if (currentLightboxIndex < 0) currentLightboxIndex = 0;
  if (currentLightboxIndex >= images.length) currentLightboxIndex = images.length - 1;

  const currentImg = images[currentLightboxIndex];
  currentMediaId = currentImg ? currentImg.getAttribute('data-media-id') : null;
  lightboxImg.src = currentImg.src;
  lightboxImg.alt = currentImg.alt || 'Full-size media preview';

  const titleEl = document.getElementById('media-lightbox-title');
  if (titleEl) {
    const mediaTitle = currentImg ? (currentImg.getAttribute('data-media-title') || '') : '';
    titleEl.textContent = mediaTitle;
    titleEl.style.display = mediaTitle ? '' : 'none';
  }

  const openBtn = document.getElementById('media-lightbox-open');
  if (openBtn) {
    openBtn.href = currentMediaId ? mediaDetailsUrl(currentMediaId) : '#';
  }

  const mediaIdInput = document.getElementById('media-lightbox-media-id');
  if (mediaIdInput) {
    mediaIdInput.value = currentMediaId || '';
  }
  const returnToInput = document.getElementById('media-lightbox-return-to');
  if (returnToInput) {
    returnToInput.value = window.location.pathname + window.location.search;
  }

  if (prevBtn) {
    prevBtn.disabled = currentLightboxIndex <= 0;
  }
  if (nextBtn) {
    nextBtn.disabled = currentLightboxIndex >= images.length - 1;
  }
};

window.mediaLightboxPrev = () => {
  const images = getImageItems();
  if (currentLightboxIndex > 0) {
    currentLightboxIndex--;
    window.updateMediaLightbox();
  }
};

window.mediaLightboxNext = () => {
  const images = getImageItems();
  if (currentLightboxIndex < images.length - 1) {
    currentLightboxIndex++;
    window.updateMediaLightbox();
  }
};

document.addEventListener('keydown', (evt) => {
  const dialog = document.getElementById('media-lightbox');
  if (!dialog || !dialog.open) return;

  const activeElement = document.activeElement;
  const tagName = activeElement ? activeElement.tagName.toLowerCase() : '';
  if (tagName === 'input' || tagName === 'textarea' || tagName === 'select') {
    return;
  }

  if (evt.key === 'ArrowLeft' || evt.key === 'h' || evt.key === 'H') {
    evt.preventDefault();
    window.mediaLightboxPrev();
  } else if (evt.key === 'ArrowRight' || evt.key === 'l' || evt.key === 'L') {
    evt.preventDefault();
    window.mediaLightboxNext();
  } else if (evt.key === 'Enter') {
    evt.preventDefault();
    if (currentMediaId) {
      window.location.href = mediaDetailsUrl(currentMediaId);
    }
  }
});
