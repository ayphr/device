import styles from './Button.module.css';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'danger';
  fullWidth?: boolean;
  isLoading?: boolean;
}

export const Button = ({
  variant = 'primary',
  fullWidth = false,
  isLoading = false,
  disabled,
  children,
  className = '',
  ...props
}: ButtonProps) => {
  const variantClass = styles[`button--${variant}`];
  const fullWidthClass = fullWidth ? styles['button--full-width'] : '';
  const buttonClasses = [styles.button, variantClass, fullWidthClass, className]
    .filter(Boolean)
    .join(' ');

  return (
    <button
      className={buttonClasses}
      disabled={disabled || isLoading}
      {...props}
    >
      {isLoading ? (
        <span className={styles.button__loader}>
          <span className={styles['button__loader-spinner']} />
        </span>
      ) : (
        children
      )}
    </button>
  );
};
