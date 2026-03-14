import type { KeyboardEvent } from 'react'
import { useEffect, useId, useRef, useState } from 'react'
import { Check, ChevronDown } from 'lucide-react'

export interface ComboboxOption {
  value: string
  label: string
}

interface ComboboxProps {
  ariaLabel: string
  className?: string
  disabled?: boolean
  onChange: (value: string) => void
  options: ComboboxOption[]
  value: string
}

export function Combobox({
  ariaLabel,
  className,
  disabled = false,
  onChange,
  options,
  value,
}: ComboboxProps) {
  const listboxId = useId()
  const rootRef = useRef<HTMLDivElement | null>(null)
  const triggerRef = useRef<HTMLButtonElement | null>(null)
  const optionRefs = useRef<Array<HTMLButtonElement | null>>([])
  const selectedIndex = Math.max(
    options.findIndex((option) => option.value === value),
    0,
  )
  const selectedOption = options[selectedIndex]
  const [isOpen, setIsOpen] = useState(false)
  const [activeIndex, setActiveIndex] = useState(selectedIndex)

  useEffect(() => {
    if (!isOpen) {
      return
    }

    const activeOption = optionRefs.current[activeIndex]
    activeOption?.focus()
    activeOption?.scrollIntoView({ block: 'nearest' })
  }, [activeIndex, isOpen])

  useEffect(() => {
    if (!isOpen) {
      return
    }

    function handlePointerDown(event: PointerEvent) {
      if (rootRef.current?.contains(event.target as Node)) {
        return
      }

      setIsOpen(false)
    }

    window.addEventListener('pointerdown', handlePointerDown)
    return () => {
      window.removeEventListener('pointerdown', handlePointerDown)
    }
  }, [isOpen])

  function closeAndFocusTrigger() {
    setIsOpen(false)
    triggerRef.current?.focus()
  }

  function openAt(index: number) {
    if (disabled || options.length === 0) {
      return
    }

    setActiveIndex(Math.min(Math.max(index, 0), options.length - 1))
    setIsOpen(true)
  }

  function selectValue(nextValue: string) {
    if (nextValue !== value) {
      onChange(nextValue)
    }

    closeAndFocusTrigger()
  }

  function handleTriggerKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (disabled) {
      return
    }

    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        openAt(selectedIndex)
        break
      case 'ArrowUp':
        event.preventDefault()
        openAt(selectedIndex)
        break
      case 'Enter':
      case ' ':
        event.preventDefault()
        if (isOpen) {
          closeAndFocusTrigger()
        } else {
          openAt(selectedIndex)
        }
        break
      default:
        break
    }
  }

  function handleOptionKeyDown(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    switch (event.key) {
      case 'ArrowDown':
        event.preventDefault()
        setActiveIndex((index + 1) % options.length)
        break
      case 'ArrowUp':
        event.preventDefault()
        setActiveIndex((index - 1 + options.length) % options.length)
        break
      case 'Home':
        event.preventDefault()
        setActiveIndex(0)
        break
      case 'End':
        event.preventDefault()
        setActiveIndex(options.length - 1)
        break
      case 'Enter':
      case ' ':
        event.preventDefault()
        selectValue(options[index].value)
        break
      case 'Escape':
        event.preventDefault()
        closeAndFocusTrigger()
        break
      case 'Tab':
        setIsOpen(false)
        break
      default:
        break
    }
  }

  return (
    <div
      className={`combobox ${isOpen ? 'combobox--open' : ''} ${className ?? ''}`.trim()}
      ref={rootRef}
    >
      <button
        aria-controls={listboxId}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
        aria-label={ariaLabel}
        className="combobox__trigger"
        disabled={disabled}
        onClick={() => {
          if (isOpen) {
            closeAndFocusTrigger()
            return
          }

          openAt(selectedIndex)
        }}
        onKeyDown={handleTriggerKeyDown}
        ref={triggerRef}
        type="button"
      >
        <span className="combobox__value">{selectedOption?.label ?? 'Select option'}</span>
        <ChevronDown
          aria-hidden="true"
          className={`combobox__chevron ${isOpen ? 'combobox__chevron--open' : ''}`}
          size={16}
          strokeWidth={2}
        />
      </button>

      {isOpen ? (
        <div className="combobox__menu" role="presentation">
          <div aria-label={ariaLabel} className="combobox__list" id={listboxId} role="listbox">
            {options.map((option, index) => {
              const isSelected = option.value === value

              return (
                <button
                  aria-selected={isSelected}
                  className={`combobox__option ${
                    isSelected ? 'combobox__option--selected' : ''
                  } ${index === activeIndex ? 'combobox__option--active' : ''}`}
                  key={option.value}
                  onClick={() => {
                    selectValue(option.value)
                  }}
                  onKeyDown={(event) => {
                    handleOptionKeyDown(event, index)
                  }}
                  ref={(node) => {
                    optionRefs.current[index] = node
                  }}
                  role="option"
                  tabIndex={index === activeIndex ? 0 : -1}
                  type="button"
                >
                  <span>{option.label}</span>
                  {isSelected ? (
                    <Check aria-hidden="true" className="combobox__check" size={15} strokeWidth={2.2} />
                  ) : null}
                </button>
              )
            })}
          </div>
        </div>
      ) : null}
    </div>
  )
}
