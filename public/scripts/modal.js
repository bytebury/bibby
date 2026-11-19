// Modals. Any `hx-target="#modal"` swap opens `#modal_wrapper` as a dialog;
// `closeModal()`, Escape, the underlay, or a server-sent `closeModal` event
// closes it. While it is open the rest of the page is `inert`, so Tab stays
// inside the dialog and screen readers only see it. Focus moves to its first
// field and returns to whatever opened it.
const modalWrapper = document.getElementById("modal_wrapper");
const modalDialog = document.getElementById("modal");
let modalOpener = null;

function modalIsOpen() {
    return modalWrapper.style.display !== "none";
}

// The toast and tooltip stay live: a failed submit inside the dialog shows
// its error in the toast.
function setPageInert(inert) {
    for (const element of document.body.children) {
        if (element !== modalWrapper && element.id !== "toast" && element.id !== "tooltip") {
            element.inert = inert;
        }
    }
}

document.addEventListener("htmx:afterSwap", function (event) {
    if (event.target !== modalDialog) {
        return;
    }
    if (!modalIsOpen()) {
        modalOpener = document.activeElement;
    }
    // Name the dialog after its heading.
    const title = modalDialog.querySelector("h1, h2, h3");
    if (title) {
        title.id ||= "modal_title";
        modalDialog.setAttribute("aria-labelledby", title.id);
    }
    modalWrapper.classList.remove("closing");
    modalWrapper.style.display = "flex";
    setPageInert(true);
    const field = modalDialog.querySelector(
        "[autofocus], input:not([type=hidden]), select, textarea",
    );
    (field || modalDialog).focus();
});

document.addEventListener("keydown", function (event) {
    if (!modalIsOpen()) {
        return;
    }
    if (event.key === "Escape") {
        event.preventDefault();
        closeModal();
    } else if (event.key === "Tab") {
        wrapFocus(event);
    }
});

// Tab from the dialog's last control goes back to its first (and Shift+Tab
// the other way) instead of out to the browser.
function wrapFocus(event) {
    const focusable = [...modalDialog.querySelectorAll(
        "a[href], button:not([disabled]), input:not([disabled]):not([type=hidden]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])",
    )].filter((element) => element.offsetParent !== null);
    if (focusable.length === 0) {
        event.preventDefault();
        return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    if (event.shiftKey && (active === first || active === modalDialog)) {
        event.preventDefault();
        last.focus();
    } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
    }
}

document.addEventListener("closeModal", closeModal);

/**
 * Closes the open modal, if any. Modals are opened by loading a partial into
 * it:
 *
 * ```html
 * <button hx-get="/your-link" hx-target="#modal">Load Modal</button>
 * ```
 */
function closeModal() {
    if (!modalIsOpen() || modalWrapper.classList.contains("closing")) {
        return;
    }
    const finish = function () {
        if (!modalWrapper.classList.contains("closing")) {
            return;
        }
        modalWrapper.classList.remove("closing");
        modalWrapper.style.display = "none";
        setPageInert(false);
        if (modalOpener && modalOpener.isConnected) {
            modalOpener.focus();
        }
        modalOpener = null;
    };
    modalWrapper.classList.add("closing");
    modalWrapper.addEventListener("animationend", finish, { once: true });
    // No animation runs under prefers-reduced-motion, so don't wait for one.
    setTimeout(finish, 400);
}
