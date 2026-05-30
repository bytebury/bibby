let activeTooltip = null;

document.addEventListener("DOMContentLoaded", function () {
    initializeTooltips();
});

document.body.addEventListener("htmx:afterSwap", function () {
    // Hide any active tooltips
    hideTooltip();
    initializeTooltips();
});

function initializeTooltips() {
    for (const element of document.querySelectorAll("[data-tooltip]")) {
        element.addEventListener("mouseenter", showTooltip);
        element.addEventListener("mouseleave", hideTooltip);
    }
}

function showTooltip(event) {
    const target = event.currentTarget;

    const isMobile = window.matchMedia("(max-width: 748px)").matches;
    const suppressOnMobile = target.hasAttribute("data-tooltip-no-mobile") ||
        target.matches("details.menu > summary");

    if (isMobile && suppressOnMobile) {
        return;
    }

    activeTooltip = target;
    tooltip.innerHTML = activeTooltip.dataset.tooltip;

    tooltip.classList.add("measure");
    const tooltipRect = tooltip.getBoundingClientRect();
    tooltip.classList.remove("measure");

    const rect = activeTooltip.getBoundingClientRect();
    const scrollX = window.scrollX;
    const windowWidth = window.innerWidth;

    const finalTop = rect.bottom + window.scrollY + 4;
    const preferredLeft = rect.left + scrollX + rect.width / 2 -
        tooltipRect.width / 2;

    let finalLeft;

    if (preferredLeft < scrollX) {
        finalLeft = rect.left + scrollX;
    } else if (preferredLeft + tooltipRect.width > scrollX + windowWidth) {
        finalLeft = rect.right + scrollX - tooltipRect.width;
    } else {
        finalLeft = preferredLeft;
    }

    tooltip.style.top = `${finalTop}px`;
    tooltip.style.left = `${finalLeft}px`;

    tooltip.classList.add("show");
}

function hideTooltip() {
    tooltip.classList.remove("show");
    activeTooltip = null;
}