let currentLightboxIndex = -1;
let currentMediaId = null;

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

  window.updateMediaLightbox();
  dialog.showModal();
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

  const openBtn = document.getElementById('media-lightbox-open');
  if (openBtn) {
    openBtn.href = currentMediaId ? `/media/details?id=${currentMediaId}` : '#';
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
      window.location.href = `/media/details?id=${currentMediaId}`;
    }
  }
});
