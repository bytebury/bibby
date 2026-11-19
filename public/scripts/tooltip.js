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

// A swap can remove the hovered element without a mouseout.
document.body.addEventListener("htmx:afterSwap", hideTooltip);

function showTooltip(target) {
    const isMobile = window.matchMedia("(max-width: 748px)").matches;
    const suppressOnMobile = target.hasAttribute("data-tooltip-no-mobile") ||
        target.matches("details.menu > summary");

    if (isMobile && suppressOnMobile) {
        return;
    }

    activeTooltip = target;
    tooltip.textContent = activeTooltip.dataset.tooltip;

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
