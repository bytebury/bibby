// Delegated from `document`, so elements HTMX swaps in need no rebinding.
let activeTooltip = null;

document.addEventListener("mouseover", function (event) {
    const target = event.target.closest("[data-tooltip]");
    if (target && target !== activeTooltip) {
        showTooltip(target);
    }
});

document.addEventListener("mouseout", function (event) {
    if (activeTooltip && !activeTooltip.contains(event.relatedTarget)) {
        hideTooltip();
    }
});

// Keyboard focus shows the same tooltip as hovering, and Escape dismisses it.
document.addEventListener("focusin", function (event) {
    const target = event.target.closest("[data-tooltip]");
    if (target) {
        showTooltip(target);
    }
});

document.addEventListener("focusout", function (event) {
    if (event.target === activeTooltip) {
        hideTooltip();
    }
});

document.addEventListener("keydown", function (event) {
    if (event.key === "Escape") {
        hideTooltip();
    }
});

// A swap can remove the hovered element without a mouseout.
document.body.addEventListener("htmx:afterSwap", hideTooltip);

function showTooltip(target) {
    const isMobile = window.matchMedia("(max-width: 748px)").matches;
    const suppressOnMobile = target.hasAttribute("data-tooltip-no-mobile") ||
        target.matches("details.menu > summary");

    if (isMobile && suppressOnMobile) {
        return;
    }

    if (activeTooltip && activeTooltip !== target) {
        activeTooltip.removeAttribute("aria-describedby");
    }
    activeTooltip = target;
    // Screen readers read the tooltip as the element's description.
    activeTooltip.setAttribute("aria-describedby", "tooltip");
    tooltip.textContent = activeTooltip.dataset.tooltip;

    tooltip.classList.add("measure");
    const tooltipRect = tooltip.getBoundingClientRect();
    tooltip.classList.remove("measure");

    const rect = activeTooltip.getBoundingClientRect();
    const margin = 8;
    const finalTop = rect.bottom + window.scrollY + 4;
    // Centred under the element, but kept `margin` px inside the window.
    const centred = rect.left + rect.width / 2 - tooltipRect.width / 2;
    const widest = window.innerWidth - tooltipRect.width - margin;
    const finalLeft = window.scrollX + Math.max(margin, Math.min(centred, widest));

    tooltip.style.top = `${finalTop}px`;
    tooltip.style.left = `${finalLeft}px`;

    tooltip.setAttribute("role", "tooltip");
    tooltip.classList.add("show");
}

function hideTooltip() {
    tooltip.classList.remove("show");
    tooltip.removeAttribute("role");
    activeTooltip?.removeAttribute("aria-describedby");
    activeTooltip = null;
}
