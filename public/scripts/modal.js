document.addEventListener("closeModal", function () {
    closeModal();
});


document.addEventListener('keydown', function (event) {
    if (event.key === "Escape") {
        closeModal();
    }
});

document.addEventListener("htmx:afterSwap", function (event) {
    if (event.target.id === "modal") {
        document.getElementById("modal_wrapper").style.display = "flex";
    }
});

/**
 * Closes a modal. All modals will be opened via the following way:
 *
 * ```html
 * <button hx-get="/your-link" hx-target="#modal">Load Modal</button>
 * ```
 */
function closeModal() {
    const modal = document.getElementById("modal_wrapper");

    modal.classList.add("closing");

    modal.addEventListener("animationend", function handleAnimationEnd() {
        modal.classList.remove("closing");
        modal.style.display = "none";
        modal.removeEventListener("animationend", handleAnimationEnd);
    });
}
