document.addEventListener("click", function (event) {
    document.querySelectorAll("details.menu[open]").forEach(function (menu) {
        if (!menu.contains(event.target) || event.target.closest(".menu-items")) {
            menu.open = false;
        }
    });
});

document.addEventListener("keydown", function (event) {
    if (event.key === "Escape") {
        document.querySelectorAll("details.menu[open]").forEach(function (menu) {
            menu.open = false;
        });
    }
});
