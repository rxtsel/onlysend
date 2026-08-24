import { tick } from "svelte";
import { cubicInOut } from "svelte/easing";
import { fly } from "svelte/transition";

// Soft glide: short distance, symmetric ease.
export const SLIDE_IN = { x: 40, duration: 260, easing: cubicInOut };
export const SLIDE_OUT = { x: -40, duration: 200, easing: cubicInOut };
export const SLIDE_BACK_IN = { x: -40, duration: 260, easing: cubicInOut };
export const SLIDE_BACK_OUT = { x: 40, duration: 200, easing: cubicInOut };

const HEIGHT_RELEASE_MS = 340;
/** Vertical padding added by VIEW_WRAP_CLASS (must stay in sync). */
const WRAP_PAD_Y = 8;

/**
 * Animates a grid-stacked view container between views:
 * slides the views (fly) and tweens the wrapper height so
 * different-sized views don't jump.
 *
 * Usage:
 *   const animator = createViewAnimator();
 *   <div bind:this={animator.element}
 *        class="grid items-start overflow-hidden p-[4px] -m-[4px] ..."
 *        style:height={animator.height === null ? "auto" : `${animator.height}px`}>
 *     {#if view === "a"}<div data-view="a" in:fly={SLIDE_IN} out:fly={SLIDE_OUT}>...
 */
export function createViewAnimator() {
  let element = $state<HTMLElement>();
  let height = $state<number | null>(null);

  async function transition(viewName: string, swap: () => void) {
    const el = element;
    if (!el) {
      swap();
      return;
    }

    // Lock current height so the swap doesn't jump.
    height = el.offsetHeight;
    swap();
    await tick();

    requestAnimationFrame(() => {
      const target = el.querySelector<HTMLElement>(`[data-view="${viewName}"]`);
      // Target rect excludes the wrapper's own vertical padding.
      height = target
        ? Math.ceil(target.getBoundingClientRect().height) + WRAP_PAD_Y
        : null;

      // Release back to auto once the height transition settles.
      setTimeout(() => (height = null), HEIGHT_RELEASE_MS);
    });
  }

  return {
    get element() {
      return element;
    },
    set element(value: HTMLElement | undefined) {
      element = value;
    },
    get height() {
      return height;
    },
    transition,
  };
}

/** Wrapper classes for an animated view container. */
export const VIEW_WRAP_CLASS =
  "grid items-start overflow-hidden transition-[height] duration-300 ease-in-out p-[4px] -m-[4px]";
