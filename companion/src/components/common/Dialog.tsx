import type { ReactNode } from 'react';
import { Modal } from './Modal';

export interface AlertProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  variant?: 'info' | 'success' | 'warning' | 'error';
  actionText?: string;
  onAction?: () => void;
}

export const Alert = ({
  isOpen,
  onClose,
  title,
  children,
  actionText = 'OK',
  onAction,
}: AlertProps) => {
  const handleConfirm = () => {
    onAction?.();
    onClose();
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={title}
      confirmText={actionText}
      onConfirm={handleConfirm}
      showCancel={false}
      size="sm"
    >
      {children}
    </Modal>
  );
};

export interface ConfirmDialogProps {
  isOpen: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  title: string;
  children: ReactNode;
  confirmText?: string;
  cancelText?: string;
  isDangerous?: boolean;
  isLoading?: boolean;
}

export const ConfirmDialog = ({
  isOpen,
  onConfirm,
  onCancel,
  title,
  children,
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  isDangerous = false,
  isLoading = false,
}: ConfirmDialogProps) => {
  return (
    <Modal
      isOpen={isOpen}
      onClose={onCancel}
      onConfirm={onConfirm}
      title={title}
      confirmText={confirmText}
      cancelText={cancelText}
      showCancel={true}
      isDangerous={isDangerous}
      isLoading={isLoading}
      size="sm"
    >
      {children}
    </Modal>
  );
};
