const DEFAULT_MS = 2400;

export function createToast(el: HTMLElement) {
  let timer: number | undefined;
  return (message: string, ms = DEFAULT_MS) => {
    el.textContent = message;
    el.hidden = false;
    clearTimeout(timer);
    timer = window.setTimeout(() => (el.hidden = true), ms);
  };
}
