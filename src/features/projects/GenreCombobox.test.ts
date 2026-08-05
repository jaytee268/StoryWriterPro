// @vitest-environment jsdom
import { act, createElement, useState, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import { GenreCombobox } from './GenreCombobox';
import { GenreMultiSelect } from './GenreMultiSelect';

const testGlobal = globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean };
testGlobal.IS_REACT_ACT_ENVIRONMENT = true;

const roots: Array<{ root: Root; container: HTMLDivElement }> = [];

afterEach(() => {
  roots.splice(0).forEach(({ root, container }) => {
    act(() => root.unmount());
    container.remove();
  });
});

async function render(element: ReactNode): Promise<HTMLDivElement> {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  roots.push({ root, container });
  await act(async () => root.render(element));
  return container;
}

function click(element: Element): Promise<void> {
  return act(async () => { element.dispatchEvent(new MouseEvent('click', { bubbles: true })); });
}

function typeInto(input: HTMLInputElement, value: string): Promise<void> {
  return act(async () => {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
    setter?.call(input, value);
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

function ComboboxHarness() {
  const [value, setValue] = useState('');
  return createElement(GenreCombobox, { value, onChange: setValue });
}

function MultiSelectHarness() {
  const [value, setValue] = useState<string[]>([]);
  return createElement(GenreMultiSelect, { value, onChange: setValue });
}

describe('Genre-Auswahl im neuen Projekt-Onboarding', () => {
  it('öffnet das Hauptgenre als durchsuchbare Combobox und wählt genau ein Genre', async () => {
    const container = await render(createElement(ComboboxHarness));
    const trigger = container.querySelector('[role="combobox"]');
    expect(trigger).not.toBeNull();
    await click(trigger!);
    const search = container.querySelector('[role="searchbox"]') as HTMLInputElement;
    await typeInto(search, 'Krimi');
    const option = [...container.querySelectorAll('[role="option"]')].find((item) => item.textContent?.includes('Krimi'));
    expect(option).not.toBeUndefined();
    await click(option!);
    expect(trigger?.textContent).toContain('Krimi');
    await click(container.querySelector('[aria-label="Hauptgenre entfernen"]')!);
    expect(trigger?.textContent).toContain('Genre auswählen');
  });

  it('schließt mit Escape, zeigt Nebengenres als Chips und entfernt einen Chip', async () => {
    const container = await render(createElement(MultiSelectHarness));
    const trigger = container.querySelector('[role="combobox"]');
    await click(trigger!);
    const option = [...container.querySelectorAll('[role="option"]')].find((item) => item.textContent?.includes('Krimi'));
    await click(option!);
    expect(container.querySelector('.genre-chip')?.textContent).toContain('Krimi');
    const remove = container.querySelector('.genre-chip button');
    await click(remove!);
    expect(container.querySelector('.genre-chip')).toBeNull();
    const search = container.querySelector('[role="searchbox"]');
    expect(search).not.toBeNull();
    await act(async () => search?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })));
    expect(container.querySelector('[role="searchbox"]')).toBeNull();
  });
});
