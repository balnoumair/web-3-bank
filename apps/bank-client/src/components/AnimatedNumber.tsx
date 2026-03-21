import { createSignal, createEffect, on, type Component } from 'solid-js';

interface AnimatedNumberProps {
  value: number;
  duration?: number;
  prefix?: string;
  decimals?: number;
  class?: string;
}

const AnimatedNumber: Component<AnimatedNumberProps> = (props) => {
  const [displayed, setDisplayed] = createSignal(0);
  let animationFrame: number | undefined;

  createEffect(
    on(
      () => props.value,
      (target) => {
        const start = displayed();
        const diff = target - start;
        const duration = props.duration ?? 600;
        const startTime = performance.now();

        if (animationFrame) cancelAnimationFrame(animationFrame);

        const step = (now: number) => {
          const elapsed = now - startTime;
          const progress = Math.min(elapsed / duration, 1);
          // Ease out cubic
          const eased = 1 - Math.pow(1 - progress, 3);
          setDisplayed(start + diff * eased);

          if (progress < 1) {
            animationFrame = requestAnimationFrame(step);
          }
        };

        animationFrame = requestAnimationFrame(step);
      },
    ),
  );

  const formatted = () => {
    const d = props.decimals ?? 2;
    return new Intl.NumberFormat('en-US', {
      minimumFractionDigits: d,
      maximumFractionDigits: d,
    }).format(displayed());
  };

  return (
    <span class={props.class}>
      {props.prefix ?? ''}
      {formatted()}
    </span>
  );
};

export default AnimatedNumber;
