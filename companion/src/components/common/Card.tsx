import type { ReactNode } from 'react';
import styles from './Card.module.css';

export interface CardProps {
  children: ReactNode;
  className?: string;
  onClick?: () => void;
  elevated?: boolean;
}

export const Card = ({ children, className = '', onClick, elevated = false }: CardProps) => {
  return (
    <div
      className={`${styles.card} ${elevated ? styles['card--elevated'] : ''} ${className}`.trim()}
      onClick={onClick}
    >
      {children}
    </div>
  );
};

export interface CardHeaderProps {
  children: ReactNode;
  className?: string;
}

export const CardHeader = ({ children, className = '' }: CardHeaderProps) => {
  return <div className={`${styles['card__header']} ${className}`.trim()}>{children}</div>;
};

export interface CardBodyProps {
  children: ReactNode;
  className?: string;
}

export const CardBody = ({ children, className = '' }: CardBodyProps) => {
  return <div className={`${styles['card__body']} ${className}`.trim()}>{children}</div>;
};

export interface CardFooterProps {
  children: ReactNode;
  className?: string;
}

export const CardFooter = ({ children, className = '' }: CardFooterProps) => {
  return <div className={`${styles['card__footer']} ${className}`.trim()}>{children}</div>;
};
