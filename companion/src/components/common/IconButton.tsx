import styles from './IconButton.module.css';

export interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  icon: React.ReactNode;
  isLoading?: boolean;
  borderless?: boolean;
}

export const IconButton = ({
  icon,
  onClick,
  disabled = false,
  isLoading = false,
  borderless = true,
  className,
  ...props
}: IconButtonProps) => {
  return (
    <button
      className={`${styles['icon-button']} ${className ?? ''} ${borderless ? styles['icon-button--borderless'] : ''}`.trim()}
      onClick={onClick}
      disabled={disabled || isLoading}
      {...props}
    >
      {isLoading ? (
        <span className={styles['icon-button__loader']}>
          <span className={styles['icon-button__loader-spinner']} />
        </span>
      ) : (
        icon
      )}
    </button>
  );
};
