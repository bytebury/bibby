// Surfaces server errors to the user. HTMX is configured to NOT swap error
// responses (responseHandling: `[45]..` => swap:false, error:true), so without
// this listener a 4xx/5xx body — e.g. a form validation message — is silently
// dropped and the form just appears to do nothing. We read the response body
// and show it in a transient toast.
//
// AppError responses are always plain-text bodies (see src/error.rs), so it is
// safe to assign them via textContent.

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
        toast.setAttribute("role", "status");
        toast.setAttribute("aria-live", "polite");
        document.body.appendChild(toast);
    }

    toast.textContent = message;
    toast.classList.add("show");

    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toast.classList.remove("show"), 5000);
}
