// Surfaces server errors to the user. HTMX is configured to NOT swap error
// responses (responseHandling: `[45]..` => swap:false, error:true), so without
// this listener a 4xx/5xx body — e.g. a form validation message — is silently
// dropped and the form just appears to do nothing. We read the response body
// and show it in a transient danger toast.
//
// AppError responses are always plain-text bodies (see src/error.rs), so the
// message is assigned via textContent and never interpreted as HTML.

const TOAST_ICON =
    '<svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">' +
    '<path fill-rule="evenodd" d="M10 18a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM8.28 7.22a.75.75 0 0 0-1.06 1.06L8.94 10l-1.72 1.72a.75.75 0 1 0 1.06 1.06L10 11.06l1.72 1.72a.75.75 0 1 0 1.06-1.06L11.06 10l1.72-1.72a.75.75 0 0 0-1.06-1.06L10 8.94 8.28 7.22z" clip-rule="evenodd" />' +
    "</svg>";
const CLOSE_ICON =
    '<svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">' +
    '<path d="M6.28 5.22a.75.75 0 0 0-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 1 0 1.06 1.06L10 11.06l3.72 3.72a.75.75 0 1 0 1.06-1.06L11.06 10l3.72-3.72a.75.75 0 0 0-1.06-1.06L10 8.94 6.28 5.22z" />' +
    "</svg>";

let toastTimer = null;

document.body.addEventListener("htmx:responseError", function (event) {
    const xhr = event.detail.xhr;
    const message = (xhr.responseText || "").trim() ||
        "Something went wrong. Please try again.";
    showToast(message);
});

function showToast(message) {
    let toast = document.getElementById("toast");
    if (!toast) {
        toast = document.createElement("div");
        toast.id = "toast";
        toast.className = "toast";
        document.body.appendChild(toast);
    }

    toast.setAttribute("role", "alert");
    toast.setAttribute("aria-live", "assertive");

    const icon = document.createElement("div");
    icon.className = "toast-icon";
    icon.innerHTML = TOAST_ICON;

    const body = document.createElement("div");
    body.className = "toast-body";

    const title = document.createElement("p");
    title.className = "toast-title";
    title.textContent = "Something went wrong";

    const text = document.createElement("p");
    text.className = "toast-message";
    text.textContent = message;

    body.append(title, text);

    const close = document.createElement("button");
    close.type = "button";
    close.className = "toast-close";
    close.setAttribute("aria-label", "Dismiss");
    close.innerHTML = CLOSE_ICON;
    close.addEventListener("click", hideToast);

    toast.replaceChildren(icon, body, close);
    // Force a reflow so re-triggering an already-visible toast replays the
    // slide-in transition instead of snapping.
    toast.classList.remove("show");
    void toast.offsetWidth;
    toast.classList.add("show");

    clearTimeout(toastTimer);
    toastTimer = setTimeout(hideToast, 6000);
}

function hideToast() {
    const toast = document.getElementById("toast");
    if (toast) {
        toast.classList.remove("show");
    }
    clearTimeout(toastTimer);
}
