(() => {
  "use strict";

  const EXPANDED_CLASS = "is-expanded";

  function opensInNewTab(event) {
    return (
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    );
  }

  function expand(figure, image, link) {
    if (!image.dataset.collapsedSrc) {
      image.dataset.collapsedSrc = image.getAttribute("src");
    }

    image.src = link.href;
    figure.classList.add(EXPANDED_CLASS);
    link.setAttribute("aria-expanded", "true");
  }

  function collapse(figure, image, link) {
    const collapsed = image.dataset.collapsedSrc;
    if (collapsed) image.src = collapsed;

    figure.classList.remove(EXPANDED_CLASS);
    link.setAttribute("aria-expanded", "false");
  }

  function keepFigureInView(figure) {
    if (figure.getBoundingClientRect().top < 0) {
      figure.scrollIntoView({ block: "start" });
    }
  }

  function toggleMedia(event) {
    if (opensInNewTab(event)) return;

    const link = event.currentTarget;
    const figure = link.closest(".post-media");
    const image = link.querySelector("img");
    if (!figure || !image) return;

    event.preventDefault();

    if (figure.classList.contains(EXPANDED_CLASS)) {
      collapse(figure, image, link);
      keepFigureInView(figure);
    } else {
      expand(figure, image, link);
    }
  }

  function install() {
    for (const link of document.querySelectorAll(".post-media a")) {
      link.setAttribute("aria-expanded", "false");
      link.addEventListener("click", toggleMedia);
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", install, { once: true });
  } else {
    install();
  }
})();
