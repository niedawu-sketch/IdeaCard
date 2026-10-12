import type { ButtonHTMLAttributes } from 'react'

type ButtonVariant = 'primary' | 'secondary' | 'ghost'
type ButtonSize = 'small' | 'medium'

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant
  size?: ButtonSize
}

export function Button({
  className = '',
  type = 'button',
  variant = 'primary',
  size = 'medium',
  ...props
}: ButtonProps) {
  const classes = [
    'button',
    `button-${variant}`,
    `button-${size}`,
    className,
  ]
    .filter(Boolean)
    .join(' ')

  return <button className={classes} type={type} {...props} />
}
