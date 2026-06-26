// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Card } from './Card';

describe('Card (v0.119 Cursor-merge)', () => {
  it('renders children inside content area', () => {
    render(<Card>child content</Card>);
    expect(screen.getByText('child content')).toBeInTheDocument();
  });

  it('applies rounded-card (12px) per Cursor spec', () => {
    const { container } = render(<Card>x</Card>);
    const card = container.querySelector('[class*="rounded-card"]')!;
    expect(card).toBeInTheDocument();
    expect(card.className).toContain('rounded-card');
    // 向后兼容:不再使用旧的 `rounded-lg`
    expect(card.className).not.toContain('rounded-lg');
  });

  it('applies hairline border + surface bg (no shadow)', () => {
    const { container } = render(<Card>x</Card>);
    const card = container.querySelector('[class*="rounded-card"]')!;
    expect(card.className).toContain('border');
    expect(card.className).toContain('bg-surface');
    expect(card.className).not.toContain('shadow-');
  });

  it('renders title as h3 when provided', () => {
    render(<Card title="My Card">body</Card>);
    const h3 = screen.getByRole('heading', { level: 3, name: 'My Card' });
    expect(h3).toBeInTheDocument();
    expect(h3.className).toContain('font-semibold');
  });

  it('renders description when provided', () => {
    render(
      <Card title="T" description="D">
        body
      </Card>,
    );
    expect(screen.getByText('D')).toBeInTheDocument();
  });

  it('does not render title row if neither title nor action provided', () => {
    const { container } = render(<Card>body</Card>);
    // 应该只有 1 个外层 div + 内容 div(无标题行)
    expect(container.children.length).toBe(1);
  });

  it('renders action when provided', () => {
    render(
      <Card title="T" action={<button>Act</button>}>
        body
      </Card>,
    );
    expect(screen.getByRole('button', { name: 'Act' })).toBeInTheDocument();
  });

  it('default padding is md (p-4)', () => {
    const { container } = render(<Card>body</Card>);
    const content = container.querySelector('.p-4');
    expect(content).toBeInTheDocument();
  });

  it('supports padding="sm" → p-3', () => {
    const { container } = render(<Card padding="sm">body</Card>);
    expect(container.querySelector('.p-3')).toBeInTheDocument();
  });

  it('supports padding="lg" → p-5', () => {
    const { container } = render(<Card padding="lg">body</Card>);
    expect(container.querySelector('.p-5')).toBeInTheDocument();
  });

  it('supports padding="none" → p-0', () => {
    const { container } = render(<Card padding="none">body</Card>);
    expect(container.querySelector('.p-0')).toBeInTheDocument();
  });

  it('passes through additional className', () => {
    const { container } = render(<Card className="my-extra-class">x</Card>);
    const card = container.firstChild as HTMLElement;
    expect(card.className).toContain('my-extra-class');
  });
});
