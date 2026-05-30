document.body.addEventListener("htmx:afterSwap", function () {
    renderTimes();
});

function renderTimes() {
    document.querySelectorAll('time[data-utc]').forEach(el => {
        const raw = el.getAttribute('data-utc');
        // Convert "2026-02-28 15:16:45.880682 UTC" to ISO format
        const iso = raw.replace(" ", "T").replace(" UTC", "Z");
        const date = new Date(iso);

        if (!isNaN(date)) {
            el.textContent = new Intl.DateTimeFormat(navigator.language, {
                dateStyle: 'long',
                timeStyle: 'short'
            }).format(date);
        }
    });
}

renderTimes();
