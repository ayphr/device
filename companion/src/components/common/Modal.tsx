import { type ReactNode, useEffect } from 'react';
import { Button } from './Button';
import { IconX } from '@tabler/icons-react';
import styles from './Modal.module.css';

export interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm?: () => void;
  title?: string;
  children?: ReactNode;
  confirmText?: string;
  cancelText?: string;
  showCancel?: boolean;
  isDangerous?: boolean;
  isLoading?: boolean;
  disableClose?: boolean;
  size?: 'sm' | 'md' | 'lg';
}

export const Modal = ({
  isOpen,
  onClose,
  onConfirm,
  title,
  children,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  showCancel = true,
  isDangerous = false,
  isLoading = false,
  disableClose = false,
  size = 'md',
}: ModalProps) => {
  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }

    return () => {
      document.body.style.overflow = '';
    };
  }, [isOpen]);

  if (!isOpen) return null;

  const handleBackdropClick = (e: React.MouseEvent) => {
    if (!disableClose && e.target === e.currentTarget) {
      onClose();
    }
  };

  const handleConfirm = () => {
    onConfirm?.();
    onClose();
  };

  return (
    <div className={styles['modal-backdrop']} onClick={handleBackdropClick}>
      <div className={styles.modal + ' ' + styles['modal--' + size]}>
        {title && (
          <div className={styles['modal__header']}>
            <h2 className={styles['modal__title']}>{title}</h2>
            <button
              className={styles['modal__close']}
              onClick={onClose}
              aria-label="Close modal"
              disabled={disableClose}
            >
              <IconX size={20} />
            </button>
          </div>
        )}
        {children && <div className={styles['modal__body']}>{children}</div>}
        {(showCancel || onConfirm) && (
          <div className={styles['modal__footer']}>
            {showCancel && (
              <Button variant="secondary" onClick={onClose}>
                {cancelText}
              </Button>
            )}
            {onConfirm && (
              <Button
                variant={isDangerous ? 'danger' : 'primary'}
                onClick={handleConfirm}
                isLoading={isLoading}
              >
                {confirmText}
              </Button>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
