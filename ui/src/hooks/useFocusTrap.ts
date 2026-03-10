import { useEffect, RefObject } from "react";

const FOCUSABLE_ELEMENTS =
    'a[href], button:not([disabled]), textarea:not([disabled]), input[type="text"]:not([disabled]), input[type="radio"]:not([disabled]), input[type="checkbox"]:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function useFocusTrap(
    ref: RefObject<HTMLElement | null>,
    isActive: boolean = true,
    autoFocus: boolean = true,
    restoreFocus: boolean = true
) {
    useEffect(() => {
        if (!isActive || !ref.current) return;

        const container = ref.current;
        const triggerElement = document.activeElement as HTMLElement;

        const getFocusableElements = () => {
            return Array.from(
                container.querySelectorAll<HTMLElement>(FOCUSABLE_ELEMENTS)
            ).filter((el) => {
                // Ensure elements are actually visible and focusable
                const bounds = el.getBoundingClientRect();
                return bounds.width > 0 || bounds.height > 0 || el.getClientRects().length > 0;
            });
        };

        const focusableElements = getFocusableElements();

        if (autoFocus && focusableElements.length > 0) {
            // Some specific autofocus logic if an element has autofocus attribute
            const autofocusElement = container.querySelector('[autofocus]') as HTMLElement;
            if (autofocusElement) {
                autofocusElement.focus();
            } else {
                focusableElements[0].focus();
            }
        } else if (autoFocus) {
            container.focus(); // fallback if container itself is focusable
        }

        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.key !== "Tab") return;

            const elements = getFocusableElements();
            if (elements.length === 0) {
                e.preventDefault();
                return;
            }

            const firstElement = elements[0];
            const lastElement = elements[elements.length - 1];

            if (e.shiftKey) {
                if (document.activeElement === firstElement || document.activeElement === container) {
                    e.preventDefault();
                    lastElement.focus();
                }
            } else {
                if (document.activeElement === lastElement || document.activeElement === container) {
                    e.preventDefault();
                    firstElement.focus();
                }
            }
        };

        container.addEventListener("keydown", handleKeyDown);

        return () => {
            container.removeEventListener("keydown", handleKeyDown);
            if (restoreFocus && triggerElement && triggerElement.focus) {
                triggerElement.focus();
            }
        };
    }, [isActive, ref, autoFocus, restoreFocus]);
}
