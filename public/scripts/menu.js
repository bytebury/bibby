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
            // Focus inside a closing menu would land on a hidden item, so it
            // goes back to the button that opened it.
            const hadFocus = menu.contains(document.activeElement);
            menu.open = false;
            if (hadFocus) {
                menu.querySelector("summary").focus();
            }
        });
    }
});
