import type { Component } from 'solid-js';

interface SkeletonProps {
  class?: string;
  width?: string;
  height?: string;
}

const Skeleton: Component<SkeletonProps> = (props) => {
  return (
    <div
      class={`skeleton ${props.class ?? ''}`}
      style={{
        width: props.width ?? '100%',
        height: props.height ?? '1rem',
      }}
    />
  );
};

export default Skeleton;
