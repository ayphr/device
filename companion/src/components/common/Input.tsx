import { useId } from 'react';
import type { InputHTMLAttributes } from 'react';
import styles from './Input.module.css';

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  helperText?: string;
  fullWidth?: boolean;
}

export const Input = ({
  label,
  error,
  helperText,
  fullWidth = false,
  className = '',
  ...props
}: InputProps) => {
  const generatedId = useId();
  const inputId = props.id ?? generatedId;

  return (
    <div className={`${styles['input-wrapper']} ${fullWidth ? styles['input-wrapper--full-width'] : ''}`.trim()}>
      {label && (
        <label htmlFor={inputId} className={styles['input-label']}>
          {label}
        </label>
      )}
      <input
        id={inputId}
        className={`${styles.input} ${error ? styles['input--error'] : ''} ${className}`.trim()}
        {...props}
      />
      {error && <span className={styles['input-error']}>{error}</span>}
      {helperText && !error && <span className={styles['input-helper']}>{helperText}</span>}
    </div>
  );
};
