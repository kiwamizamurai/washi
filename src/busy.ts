export interface BusySink {
  show(message: string): void;
  hide(): void;
}

export const BUSY_DELAY_MS = 400;

export function createBusy(sink: BusySink, delayMs = BUSY_DELAY_MS) {
  let active = 0;
  let message = "";
  let shown = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const reveal = () => {
    shown = true;
    sink.show(message);
  };

  const settle = () => {
    active -= 1;
    if (active > 0) return;
    clearTimeout(timer);
    timer = undefined;
    if (shown) sink.hide();
    shown = false;
  };

  return async function track<T>(label: string, work: Promise<T>): Promise<T> {
    active += 1;
    message = label;
    if (shown) reveal();
    else if (timer === undefined) timer = setTimeout(reveal, delayMs);
    try {
      return await work;
    } finally {
      settle();
    }
  };
}
