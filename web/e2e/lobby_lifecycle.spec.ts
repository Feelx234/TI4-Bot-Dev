import { expect, Page, test } from '@playwright/test';

function failOnBrowserErrors(page: Page): void {
  page.on('pageerror', (error) => { throw error; });
  page.on('console', (message) => { if (message.type() === 'error' && !message.text().includes('favicon')) throw new Error(message.text()); });
}

test('creates, joins, leaves, rejoins, starts, and restores using the real server', async ({ browser }) => {
  const hostContext = await browser.newContext(); await hostContext.grantPermissions(['clipboard-read', 'clipboard-write']); const host = await hostContext.newPage(); failOnBrowserErrors(host);
  await host.goto('/'); await host.getByLabel('Players').selectOption('2'); await host.getByTestId('create-game-button').click();
  await expect(host.getByText('Game lobby')).toBeVisible();
  const hostUrl = host.url(); expect(hostUrl).not.toContain('#');
  await expect(host.getByTestId('start-game-button')).toBeDisabled();
  await host.getByTestId('ready-button').click();
  await host.getByRole('button', { name: 'Copy game URL' }).click();
  const gameUrl = await host.evaluate(() => navigator.clipboard.readText());
  const gameId = new URL(hostUrl).pathname.split('/').pop()!;
  expect(gameUrl).toBe(hostUrl);
  await expect(host.getByText(/Position 1: Player 1 \(Host\)/)).toBeVisible();
  await expect(host.locator('.lobby-panel')).not.toContainText(gameId);
  await expect(host.locator('.lobby-panel')).not.toContainText('player_');
  const guestContext = await browser.newContext(); const guest = await guestContext.newPage(); failOnBrowserErrors(guest);
  await guest.goto(gameUrl);
  expect(guest.url()).toBe(gameUrl);
  await expect(guest.getByText('Join or watch')).toBeVisible();
  await guest.getByRole('button', { name: 'Watch' }).click();
  await expect(guest.getByText('Watching as spectator')).toBeVisible();
  await guest.getByRole('button', { name: 'Join game' }).click();
  await expect(guest.getByTestId('ready-button')).toBeVisible();
  await expect(guest.getByText(/Position 2: Player 2/)).toBeVisible();
  await guest.getByRole('button', { name: 'Leave lobby' }).click();
  await expect(guest).toHaveURL(new URL('/', gameUrl).href);
  await expect(host.getByText(/Position 2: Open/)).toBeVisible();
  await guest.goto(gameUrl);
  await guest.getByRole('button', { name: 'Join game' }).click();
  await expect(guest.getByTestId('ready-button')).toBeVisible();
  await guest.getByTestId('ready-button').click();
  await expect(host.getByTestId('start-game-button')).toBeEnabled();
  await host.getByTestId('start-game-button').click();
  await expect(host.getByTestId('turn-status-bar')).toBeVisible();
  await host.reload(); await expect(host.getByTestId('turn-status-bar')).toBeVisible();
  await guest.reload(); await expect(guest.getByTestId('turn-status-bar')).toBeVisible();
  await hostContext.close(); await guestContext.close();
});
