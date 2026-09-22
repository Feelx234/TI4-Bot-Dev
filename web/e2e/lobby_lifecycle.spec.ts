import { expect, Page, test } from '@playwright/test';

function failOnBrowserErrors(page: Page): void {
  page.on('pageerror', (error) => { throw error; });
  page.on('console', (message) => { if (message.type() === 'error' && !message.text().includes('favicon')) throw new Error(message.text()); });
}

test('creates a lobby in the UI, readies both players, starts, and restores after reload', async ({ browser }) => {
  const hostContext = await browser.newContext(); await hostContext.grantPermissions(['clipboard-read', 'clipboard-write']); const host = await hostContext.newPage(); failOnBrowserErrors(host);
  await host.goto('/'); await host.getByLabel('Players').selectOption('2'); await host.getByTestId('create-game-button').click();
  await expect(host.getByText('Game lobby')).toBeVisible();
  const hostUrl = host.url(); expect(hostUrl).not.toContain('#');
  await expect(host.getByTestId('start-game-button')).toBeDisabled();
  await host.getByTestId('ready-button').click();
  await host.getByRole('button', { name: 'Copy game URL' }).click();
  const gameUrl = await host.evaluate(() => navigator.clipboard.readText());
  const gameId = new URL(hostUrl).pathname.split('/').pop()!;
  const guestContext = await browser.newContext(); const guest = await guestContext.newPage(); failOnBrowserErrors(guest);
  await guest.goto(gameUrl);
  expect(guest.url()).toBe(`http://127.0.0.1:3000/games/${gameId}`);
  await expect(guest.getByText('Choose an available seat')).toBeVisible();
  await guest.getByTestId('claim-seat-p2').click();
  await expect(guest.getByTestId('ready-button')).toBeVisible();
  await guest.getByTestId('ready-button').click();
  await expect(host.getByTestId('start-game-button')).toBeEnabled();
  await host.getByTestId('start-game-button').click();
  await expect(host.getByTestId('turn-status-bar')).toBeVisible();
  await host.reload(); await expect(host.getByTestId('turn-status-bar')).toBeVisible();
  await guest.reload(); await expect(guest.getByTestId('turn-status-bar')).toBeVisible();
  await hostContext.close(); await guestContext.close();
});
