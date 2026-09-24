import { expect, test } from '@playwright/test';
import { fallbackCases, galleryCases } from '../src/dev/decisionGalleryCases.ts';

test('all current workflow kinds open a rendered preview', async ({ page }) => {
  for (const item of galleryCases) {
    await page.goto('/dev/decisions');
    await page.getByRole('button', { name: new RegExp(`^${item.title}`) }).click();
    await expect(page.getByTestId('game-container')).toBeAttached();
    await expect(page.getByRole('region', { name: 'Preview details' })).toContainText(`Classified: ${item.workflow}`);
    await expect(page.locator('[role="dialog"]:visible')).toHaveCount(1);
  }
});

test('dev gallery exposes all workflows and an actionable empty-state fallback', async ({ page }) => {
  await page.goto('/dev/decisions');
  await expect(page.getByText('Workflow kinds (16)')).toBeVisible();
  await expect(page.getByText('Fallbacks and boundary states (6)')).toBeVisible();
  await page.getByRole('button', { name: /Empty movement Explicit finish/i }).click();
  await expect(page.getByTestId('tactical-movement-tray')).toBeVisible();
  await page.getByTestId('commit-moves-btn').click();
  await expect(page.getByRole('status')).toContainText('Local submission: done_moving (no engine transition)');
  await page.getByRole('button', { name: 'Minimize decision' }).click();
  await page.getByRole('button', { name: 'All decisions' }).click();
  await page.getByRole('button', { name: /Missing movement finish Missing explicit finish/i }).click();
  await page.getByTestId('commit-moves-btn').click();
  await expect(page.getByTestId('tactical-movement-tray').getByRole('alert')).toBeVisible();
});

test('all fallback examples open with their boundary clearly labeled', async ({ page }) => {
  for (const item of fallbackCases) {
    await page.goto('/dev/decisions');
    await page.getByRole('button', { name: new RegExp(`^${item.title}`) }).click();
    await expect(page.getByRole('region', { name: 'Preview details' })).toContainText(item.fallback!);
    await expect(page.locator('[role="dialog"]:visible')).toHaveCount(1);
  }
});
