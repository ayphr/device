import styles from './Toggle.module.css';

export interface ToggleProps {
  checked: boolean;
  onChange: () => void;
  ariaPressed?: boolean;
  color?: 'red' | 'green';
}

export function Toggle({ checked, onChange, ariaPressed, color = 'red' }: Readonly<ToggleProps>) {
  let extraClass = '';
  if (checked) {
    extraClass = styles['settings-switch--on'];
    if (color === 'green') {
      extraClass += ` ${styles['settings-switch--on__green']}`;
    }
  }

  return (
    <button
      type="button"
      className={`${styles['settings-switch']} ${extraClass}`.trim()}
      onClick={onChange}
      aria-pressed={ariaPressed ?? checked}
    >
      <span />
    </button>
  );
}
