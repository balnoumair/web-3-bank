import { Component, JSX } from 'solid-js';

interface ActionButtonProps {
    icon: JSX.Element;
    label: string;
    onClick?: () => void;
    variant?: 'primary' | 'success';
}

const ActionButton: Component<ActionButtonProps> = (props) => {
    const variant = () => props.variant || 'primary';

    return (
        <button
            onClick={props.onClick}
            class={`btn ${variant() === 'primary' ? 'btn-primary' : 'btn-success'} w-full flex items-center gap-3`}
        >
            <span class="w-5 h-5">
                {props.icon}
            </span>
            <span>{props.label}</span>
        </button>
    );
};

export default ActionButton;
