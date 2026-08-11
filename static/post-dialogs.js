(() => {
  "use strict";

  function findDialog(link) {
    const id = link.dataset.dialog;
    if (!id) return null;

    const dialog = document.getElementById(id);
    return dialog instanceof HTMLDialogElement ? dialog : null;
  }

  function quotePost(dialog, postNumber) {
    if (!postNumber) return;

    const body = dialog.querySelector("textarea");
    if (!body) return;

    const quote = `>>${postNumber}`;
    if (!body.value.includes(quote)) {
      const separator = body.value.length > 0 ? "\n" : "";
      body.value = `${quote}\n${separator}${body.value}`;
    }

    body.focus();
    body.setSelectionRange(body.value.length, body.value.length);
  }

  function openDialog(event) {
    const link = event.currentTarget;
    const dialog = findDialog(link);
    if (!dialog || typeof dialog.showModal !== "function") return;

    event.preventDefault();

    if (!dialog.open) dialog.showModal();
    quotePost(dialog, link.dataset.quote);
  }

  function closeDialog(event) {
    const dialog = event.currentTarget.closest("dialog");
    if (!dialog || !dialog.open) return;

    event.preventDefault();
    dialog.close();
  }

  function install() {
    for (const link of document.querySelectorAll(".post-action[data-dialog]")) {
      link.addEventListener("click", openDialog);
    }

    for (const link of document.querySelectorAll(".dialog-close")) {
      link.addEventListener("click", closeDialog);
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", install, { once: true });
  } else {
    install();
  }
})();
