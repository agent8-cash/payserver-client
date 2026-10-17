import { test, expect } from '@playwright/test';
import { readFileSync } from 'fs';
import path from 'path';

/**
 * Layout regressions in the real stylesheet, with no server and no auth.
 *
 * `styles.css` is over 5000 lines and layers two design systems, so it contains
 * several pairs of rules with IDENTICAL specificity where the later one wins by
 * source order alone. That has produced three separate user-visible bugs, and
 * none of them could fail a normal test: the app compiles, renders, and is
 * simply wrong.
 *
 * These tests load the actual stylesheet against the actual class combinations
 * and assert computed style. No auth means no rate limit, and no server means
 * they run in milliseconds - which matters, because the alternative is noticing
 * from a screenshot.
 */
// Resolved from this file, not the process cwd. Playwright can be run from the
// repo root with `--config e2e/playwright.config.ts`, and a relative path there
// makes the whole spec die during collection with ENOENT - reported as "no
// tests found" rather than as a broken path.
//
// The @import of Google Fonts is stripped because these tests exist to be fast
// and network-free: leaving it in makes every setContent fetch fonts.googleapis
// .com (542ms per call measured, versus 28ms without) and hang to the test
// timeout on a runner with no egress. The font changes no measurement here -
// input and select are 36px with or without Inter.
//
// Matched to end of LINE, not to the first semicolon. The font URL contains
// semicolons of its own (`wght@400;500;600;700`), so `@import[^;]+;` cuts
// inside the url() and leaves `500;600;700&display=swap');` behind, which
// swallows the :root block that defines --space-3 - every `gap` in the file
// then computes to `normal` and card spacing silently becomes 0. Measured: the
// naive strip turned a 12px gap into 0.
const CSS = readFileSync(path.join(__dirname, '../styles.css'), 'utf8')
  .replace(/^\s*@import\b.*$/gm, '');

async function displays(page: import('@playwright/test').Page, html: string, sels: string[]) {
  await page.setContent(`<style>${CSS}</style>${html}`);
  const out: Record<string, string> = {};
  for (const s of sels) out[s] = await page.locator(s).evaluate((el) => getComputedStyle(el).display);
  return out;
}

/**
 * `.mobile-only { display: none }` sits at line ~2714; `.invoices-cards` and
 * `.payments-cards` set `display: flex` at ~2729 and ~3189. Same specificity,
 * so the component won and the card list rendered on desktop UNDER the table -
 * every table showed its rows twice.
 */
test.describe('responsive visibility', () => {
  const MARKUP = `
    <div class="payments-table-container desktop-only"><table class="payments-table"><tr><td>row</td></tr></table></div>
    <div class="payments-cards mobile-only"><div class="payment-card">card</div></div>
    <div class="invoices-cards mobile-only"><div>card</div></div>`;

  test('desktop shows the table and hides both card lists', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    const d = await displays(page, MARKUP, [
      '.payments-table-container', '.payments-cards', '.invoices-cards',
    ]);
    expect(d['.payments-table-container']).not.toBe('none');
    expect(d['.payments-cards'], 'payment cards must not double the table').toBe('none');
    expect(d['.invoices-cards'], 'invoice cards must not double the table').toBe('none');
  });

  test('mobile shows the card lists and hides the table', async ({ page }) => {
    await page.setViewportSize({ width: 500, height: 900 });
    const d = await displays(page, MARKUP, [
      '.payments-table-container', '.payments-cards', '.invoices-cards',
    ]);
    expect(d['.payments-table-container']).toBe('none');
    // `flex`, not merely "not none". Forcing the utility to `display: block`
    // showed the list while silently killing its `gap`, and an assertion of
    // `!== 'none'` passed the whole time.
    expect(d['.payments-cards'], 'card list must stay a flex column').toBe('flex');
    expect(d['.invoices-cards'], 'card list must stay a flex column').toBe('flex');
  });

  /**
   * The above checks the declared display; this checks the spacing it is for.
   * A card list can be visible, be flex, and still render its cards flush if a
   * later rule wins - which is the failure mode this whole file exists for.
   */
  test('mobile cards are actually spaced apart', async ({ page }) => {
    await page.setViewportSize({ width: 500, height: 900 });
    await page.setContent(`<style>${CSS}</style>
      <div class="payments-cards mobile-only">
        <div class="payment-card">a</div><div class="payment-card">b</div>
      </div>`);
    const gap = await page.locator('.payments-cards').evaluate((el) => {
      const cards = el.querySelectorAll('.payment-card');
      const a = cards[0].getBoundingClientRect();
      const b = cards[1].getBoundingClientRect();
      return Math.round(b.top - a.bottom);
    });
    expect(gap, 'cards must not sit flush against each other').toBeGreaterThan(0);
  });
});

/**
 * `.form-group + .form-group { margin-top }` is a stacked-form rule that also
 * applied inside containers with their own `gap`, pushing every second field
 * 16px below the first. Reported twice: Amount vs Currency in the create-invoice
 * modal, and Account Created vs Last Login in Settings.
 */
test.describe('form field alignment', () => {
  const cases: [string, string][] = [
    ['.form-row', `<div class="form-row">
       <div class="form-group form-group-grow"><label class="form-label">A</label><input class="form-input"></div>
       <div class="form-group"><label class="form-label">B</label><select class="form-input"><option>x</option></select></div>
     </div>`],
    ['.settings-grid', `<div class="settings-grid">
       <div class="form-group"><label class="form-label">A</label><div class="form-static">v</div></div>
       <div class="form-group"><label class="form-label">B</label><div class="form-static">v</div></div>
     </div>`],
  ];

  for (const [container, markup] of cases) {
    test(`${container} aligns its fields on one line`, async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.setContent(`<style>${CSS}</style><div style="width:900px;padding:24px">${markup}</div>`);
      const groups = page.locator(`${container} > .form-group`);
      const a = await groups.nth(0).boundingBox();
      const b = await groups.nth(1).boundingBox();
      expect(Math.round(b!.y - a!.y), `${container} second field must not sit lower`).toBe(0);
    });
  }

  test('an input and a select are the same height side by side', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.setContent(
      `<style>${CSS}</style><div style="width:900px"><input class="form-input"><select class="form-input"><option>x</option></select></div>`);
    const i = await page.locator('input.form-input').boundingBox();
    const s = await page.locator('select.form-input').boundingBox();
    expect(Math.round(s!.height - i!.height), 'select must not sag beside an input').toBe(0);
  });
});

/**
 * `.payment-row` is used for two different things: the dashboard renders each
 * recent payment as `<a class="payment-row">` and wants the flex list layout,
 * while the payments and invoice-detail tables use `<tr class="payment-row">`.
 * `display: flex` on a table row removes it from table layout, so its cells stop
 * sharing the widths the <thead> computed and every header drifts off its
 * column - measured at 600-700px of drift before this was fixed.
 */
test.describe('table column alignment', () => {
  const TABLE = `
    <div class="payments-table-container desktop-only">
      <table class="payments-table">
        <thead><tr>
          <th>Transaction</th><th>Store</th><th>Amount</th><th>Network</th>
          <th>Invoice</th><th>Status</th><th>Date</th><th></th>
        </tr></thead>
        <tbody>
          <tr class="payment-row">
            <td><div class="payment-tx-cell"><code class="tx-hash">0x1a2b…3c4d</code></div></td>
            <td><span class="payment-store">Acme Store</span></td>
            <td><span class="payment-amount">0.5 ETH</span></td>
            <td><span class="payment-network">Sepolia</span></td>
            <td><a class="payment-invoice-link">inv_123</a></td>
            <td><span class="status-badge status-confirmed">Confirmed</span></td>
            <td><span class="payment-date">Sep 7, 2026</span></td>
            <td><a class="ps-btn ps-btn-ghost ps-btn-sm ps-btn-icon">⋯</a></td>
          </tr>
        </tbody>
      </table>
    </div>`;

  test('every header sits exactly over its column', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.setContent(`<style>${CSS}</style><div style="padding:24px">${TABLE}</div>`);

    const cols = await page.evaluate(() => {
      const ths = [...document.querySelectorAll('.payments-table thead th')];
      const tds = [...document.querySelectorAll('.payments-table tbody td')];
      return ths.map((th, i) => ({
        col: (th.textContent || 'actions').trim() || 'actions',
        dx: Math.round(tds[i].getBoundingClientRect().left - th.getBoundingClientRect().left),
      }));
    });

    expect(cols.length, 'header and body must have the same number of cells').toBe(8);
    for (const { col, dx } of cols) {
      expect(dx, `"${col}" header must sit over its column`).toBe(0);
    }
  });

  test('a table row stays a table row', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.setContent(`<style>${CSS}</style>${TABLE}`);
    const display = await page.locator('.payments-table tbody tr').evaluate((el) => getComputedStyle(el).display);
    expect(display, 'flex here silently destroys column alignment').toBe('table-row');
  });
});

/**
 * A plugin table (`plugin_page.rs` renders every `PageElement::Table` as
 * `.table-container > table.table`) has no fixed column widths, so a narrow
 * viewport used to shrink columns to fit instead of scrolling - clipping a
 * long decimal (`PAID TO DATE`, a NUMERIC(38,18) sum) mid-word and wrapping a
 * status cell onto a second line. `.table` now gets a `min-width` under the
 * same breakpoint the sibling tables already use, forcing `.table-container`
 * (which sets `overflow-x: auto`) to scroll instead of the columns squeezing.
 *
 * Two things could still go wrong that a screenshot would catch and a class
 * name would not: the min-width could land on a container with no scroll
 * rule, in which case the *page* scrolls sideways instead of the table - the
 * one outcome that is explicitly not allowed - or the rule could simply not
 * apply and the old clipping would return.
 *
 * The status cell below is plain text, not a styled span: `PageElement::Table`
 * carries `rows: Vec<Vec<String>>` with no slot for a badge, and the plugin
 * that fills this table keeps only a badge's text for its status column, so
 * `<td>Active</td>` is what the live page emits, not a simplification of it.
 */
test.describe('plugin page table overflow', () => {
  const TABLE = `
    <div class="ps-page">
      <div class="ps-card">
        <div class="ps-card-body">
          <div class="table-container">
            <table class="table">
              <thead><tr><th>Plan</th><th>Status</th><th>Paid To Date</th></tr></thead>
              <tbody>
                <tr><td>Pro Monthly</td><td>Active</td><td>0.500000000000000000 USDC</td></tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>`;

  test('the table scrolls inside its own container on a narrow screen', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 800 });
    await page.setContent(`<style>${CSS}</style>${TABLE}`);

    const overflowX = await page
      .locator('.table-container')
      .evaluate((el) => getComputedStyle(el).overflowX);
    expect(overflowX, '.table-container must be the scroll boundary').toBe('auto');

    const { containerScrolls, tableWidth } = await page.locator('.table-container').evaluate((el) => ({
      containerScrolls: el.scrollWidth > el.clientWidth,
      tableWidth: el.querySelector('table')!.getBoundingClientRect().width,
    }));
    expect(containerScrolls, 'the wide table must overflow its own container').toBe(true);
    expect(tableWidth, 'table must not be squeezed narrower than its min-width').toBeGreaterThanOrEqual(500);
  });

  test('the page body does not scroll sideways when the table does', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 800 });
    await page.setContent(`<style>${CSS}</style>${TABLE}`);

    const bodyOverflows = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    );
    expect(bodyOverflows, 'the table must scroll inside .table-container, not push the page wide').toBe(false);
  });
});

/**
 * `.payment-method-row` has the identical collision: the dashboard's payment
 * methods breakdown renders each entry as `<div class="payment-method-row">`
 * and wants a flex layout, while the Payment Methods tab's table renders
 * `<tr class="payment-method-row">`. `display: flex` on that `<tr>` drops it
 * out of table layout, so its cells stop sharing the widths `<thead>`
 * computed - full-width headers over a left-bunched strip of cells.
 *
 * This suite is CSS-only by design (see playwright.config.ts): the markup
 * below is hand-authored, not rendered by the app, so it proves the stylesheet
 * behaves correctly against table markup shaped like the real thing - it does
 * not by itself prove the live component emits that shape. That question (is
 * the row's `<tr>` actually a direct child of `<tbody>`, or does something
 * hoist or wrap it) was checked separately by mounting the real
 * `PaymentMethodsTab` component in a browser against a stubbed API response
 * and inspecting the live tree, twice now on separate passes with the same
 * result; see the comment on `tr.payment-method-row` in styles.css for the
 * measurements and for why hoisting can't happen at all in a CSR app (the
 * browser algorithm that does it only runs while parsing HTML text, and this
 * app never produces any - it builds DOM nodes directly).
 */
test.describe('payment methods table column alignment', () => {
  const TABLE = `
    <div class="payment-methods-table-container">
      <table class="payment-methods-table">
        <thead><tr>
          <th>Asset</th><th>Network</th><th>Type</th>
          <th>Derivation Index</th><th>Status</th><th></th>
        </tr></thead>
        <tbody>
          <tr class="payment-method-row">
            <td><div class="payment-method-asset"><span class="payment-method-symbol">USDC</span></div></td>
            <td><span class="payment-method-network">Sepolia</span></td>
            <td><span class="payment-method-type">ERC20</span></td>
            <td><code class="payment-method-index">0</code></td>
            <td><button class="badge badge-success">Enabled</button></td>
            <td><button class="ps-btn ps-btn-ghost ps-btn-sm">Delete</button></td>
          </tr>
        </tbody>
      </table>
    </div>`;

  test('every header sits exactly over its column', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.setContent(`<style>${CSS}</style><div style="padding:24px">${TABLE}</div>`);

    const cols = await page.evaluate(() => {
      const ths = [...document.querySelectorAll('.payment-methods-table thead th')];
      const tds = [...document.querySelectorAll('.payment-methods-table tbody td')];
      return ths.map((th, i) => ({
        col: (th.textContent || 'actions').trim() || 'actions',
        dx: Math.round(tds[i].getBoundingClientRect().left - th.getBoundingClientRect().left),
      }));
    });

    expect(cols.length, 'header and body must have the same number of cells').toBe(6);
    for (const { col, dx } of cols) {
      expect(dx, `"${col}" header must sit over its column`).toBe(0);
    }
  });

  test('a table row stays a table row', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.setContent(`<style>${CSS}</style>${TABLE}`);
    const display = await page.locator('.payment-methods-table tbody tr').evaluate((el) => getComputedStyle(el).display);
    expect(display, 'flex here silently destroys column alignment').toBe('table-row');
  });
});

/**
 * `.ps-recovery-confirm` carried a copy of the `.ps-checkbox-label` rule -
 * `display: flex` with the warning background and padding. Applied to the step
 * container instead of the checkbox, it laid the four children out as four
 * columns: the title, the description, the checkbox label and the buttons, each
 * a narrow strip of text. It also nested the warning background
 * inside itself, invisibly.
 *
 * Reported twice from real use - passkey registration and wallet registration,
 * which share this component from ui-kit.
 */
test.describe('recovery confirm step', () => {
  const MARKUP = `
    <div class="ps-recovery-confirm">
      <h3 class="ps-recovery-title">Confirm Your Recovery Phrase</h3>
      <p class="ps-recovery-description">Please confirm that you have saved your recovery phrase securely.</p>
      <label class="ps-checkbox-label"><input type="checkbox" class="ps-checkbox"/><span>I have written down my recovery phrase.</span></label>
      <div class="ps-recovery-actions"><button class="ps-button ps-button-primary">Complete Setup</button></div>
    </div>`;

  test('the step stacks its children instead of laying them out as columns', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.setContent(`<style>${CSS}</style>${MARKUP}`);

    const boxes = await page.locator('.ps-recovery-confirm > *').evaluateAll((els) =>
      els.map((el) => el.getBoundingClientRect()).map((r) => ({ x: Math.round(r.x), y: Math.round(r.y) })),
    );
    expect(boxes.length).toBe(4);

    // Stacked: every child starts at the same x and strictly below the previous.
    // Side-by-side columns is the bug, and it shows up as increasing x.
    for (let i = 1; i < boxes.length; i++) {
      expect(boxes[i].x, `child ${i} shifted right — laid out as a column`).toBe(boxes[0].x);
      expect(boxes[i].y, `child ${i} did not stack below child ${i - 1}`).toBeGreaterThan(boxes[i - 1].y);
    }
  });

  test('the warning background belongs to the checkbox, not the whole step', async ({ page }) => {
    await page.setContent(`<style>${CSS}</style>${MARKUP}`);
    const step = await page.locator('.ps-recovery-confirm').evaluate((el) => getComputedStyle(el).backgroundColor);
    const label = await page.locator('.ps-checkbox-label').evaluate((el) => getComputedStyle(el).backgroundColor);

    expect(label, 'the checkbox should sit in the warning box').not.toBe('rgba(0, 0, 0, 0)');
    expect(step, 'the step container should not repeat it').toBe('rgba(0, 0, 0, 0)');
  });
});

test.describe('safe-mode banner', () => {
  // The banner is taken from the Rust source rather than retyped here. The
  // client is wasm and needs a trunk build plus an authenticated admin session
  // to reach the safe-mode branch, which this server-less suite cannot do; but
  // a hand-copied snippet would keep passing after the real markup moved to a
  // different class. Reading the `SafeModeBanner::Active` arm means a change
  // of classes there changes what is asserted here.
  const SRC = readFileSync(path.join(__dirname, '../src/pages/settings/admin.rs'), 'utf8');
  const arm = SRC.match(/SafeModeBanner::Active => view! \{\s*<div class="([^"]+)">\s*<(\w+)>/);

  test('the active arm of the real admin tab is found', () => {
    expect(arm, 'could not locate the SafeModeBanner::Active markup in admin.rs').not.toBeNull();
  });

  test('is visibly styled in the error tone rather than plain text', async ({ page }) => {
    const [, classes, heading] = arm!;
    await page.setContent(
      `<style>${CSS}</style><div class="${classes}"><${heading}>SAFE MODE</${heading}><p>plugins disabled</p></div>`,
    );
    await expect(page.locator('div').first()).toBeVisible();

    const probe = await page.evaluate(() => {
      const el = document.querySelector('div')!;
      const s = getComputedStyle(el);
      // Resolve the token through a throwaway element so the expected value is
      // in the same rgb() form as the computed one.
      const t = document.createElement('i');
      t.style.color = getComputedStyle(document.documentElement).getPropertyValue('--color-error').trim();
      document.body.appendChild(t);
      return {
        bg: s.backgroundColor,
        borderColor: s.borderTopColor,
        borderWidth: s.borderTopWidth,
        token: getComputedStyle(document.documentElement).getPropertyValue('--color-error').trim(),
        error: getComputedStyle(t).color,
        headingColor: getComputedStyle(el.firstElementChild!).color,
        pageBg: getComputedStyle(document.body).backgroundColor,
      };
    });
    expect(probe.token, '--color-error must resolve, or the colour checks compare fallbacks').not.toBe('');
    expect(probe.borderWidth, 'banner should have a border').not.toBe('0px');
    expect(probe.borderColor, 'border should be the error colour').toBe(probe.error);
    expect(probe.headingColor, 'heading should carry the error tone').toBe(probe.error);
    expect(probe.bg, 'banner background should differ from the page').not.toBe(probe.pageBg);
  });
});

/**
 * The store switcher rendered every store with no height bound, so an account
 * with dozens of them made the menu taller than the screen. The list must
 * scroll inside the menu, and the Manage link must stay reachable below it.
 */
test.describe('store switcher', () => {
  const markup = (n: number) => `
    <aside class="sidebar"><div class="store-selector">
      <button class="store-selector-btn">Store</button>
      <div class="store-dropdown open">
        <div class="store-dropdown-list">
          <button class="store-dropdown-item"><span>All Stores</span></button>
          ${Array.from({ length: n }, (_, i) =>
            `<button class="store-dropdown-item"><span title="s${i}">A very long shared store name prefix ${i}</span></button>`).join('')}
        </div>
        <div class="store-dropdown-divider"></div>
        <a class="store-dropdown-item store-dropdown-manage"><span>Manage Stores</span></a>
      </div>
    </div></aside>`;

  for (const n of [36, 2, 1, 0]) {
    test(`stays inside a laptop viewport with ${n} stores`, async ({ page }) => {
      await page.setViewportSize({ width: 1280, height: 600 });
      await page.setContent(`<style>${CSS}</style>${markup(n)}`);
      const m = await page.locator('.store-dropdown').evaluate((el) => {
        const r = el.getBoundingClientRect();
        const l = el.querySelector('.store-dropdown-list')!;
        const manage = el.querySelector('.store-dropdown-manage')!.getBoundingClientRect();
        return { bottom: r.bottom, manageBottom: manage.bottom, scrolls: l.scrollHeight > l.clientHeight };
      });
      expect(m.bottom, 'menu must end above the viewport bottom').toBeLessThanOrEqual(600);
      expect(m.manageBottom, 'Manage link must be on screen').toBeLessThanOrEqual(600);
      expect(m.scrolls, 'only a long list should scroll').toBe(n >= 10);
    });
  }
});

/**
 * A plugin's summary tiles, as the billing plugin's operator dashboard sends
 * them: four `figure` elements, each with a tone.
 *
 * What covers what, because it is not obvious and getting it wrong here means
 * trusting a test that cannot fail:
 *
 * * that the renderer emits the right classes at all is a Rust test, in
 *   `plugin_page.rs`;
 * * that every class it emits HAS a rule is also a Rust test there, which
 *   calls the four class-picking helpers rather than reading them;
 * * that the rules then produce four readably different numbers is only
 *   answerable here. A rule can exist, match, and still lose to a later one of
 *   equal specificity - the failure this whole file exists for - and no
 *   amount of checking that a selector is present can see it.
 */
test.describe('plugin summary tiles', () => {
  // The markup the Rust renderer produces for a figure element, by hand -
  // these tests have no WASM to run. Kept in the same shape so a change to the
  // renderer's structure shows up as a failure here rather than as a test that
  // measures markup nothing emits any more.
  const tile = (tone: string, label: string, value: string) => `
    <div class="plugin-figure plugin-figure-${tone}">
      <span class="plugin-figure-label">${label}</span>
      <span class="plugin-figure-value">${value}</span>
    </div>`;

  const TONES = [
    ['success', 'Active', '128'],
    ['warning', 'Past due', '6'],
    ['danger', 'Lapsed', '7'],
    ['neutral', 'Monthly recurring', '64.00 USDC'],
  ];

  const MARKUP = `<div class="plugin-grid">${TONES.map(([t, l, v]) => tile(t, l, v)).join('')}</div>`;

  // The host's own metric card, in the same page. Both of the tests below
  // compare against it rather than against a resolved token: `--text-primary`
  // is itself `var(--color-gray-900)`, and reading a chained custom property
  // back out through a throwaway element compares the tile with a value
  // reconstructed by the test instead of with the screen it has to match.
  const HOST_CARD = `
    <div class="metric-card">
      <div class="metric-label">Volume</div><div class="metric-value">1.2</div>
    </div>`;

  test('every tone colours the number, and no two agree', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.setContent(`<style>${CSS}</style>${MARKUP}${HOST_CARD}`);

    const seen: Record<string, string> = {};
    for (const [tone] of TONES) {
      seen[tone] = await page
        .locator(`.plugin-figure-${tone} .plugin-figure-value`)
        .evaluate((el) => getComputedStyle(el).color);
    }

    // Not merely "has a colour": an unstyled class still computes to the
    // inherited one, so a missing rule looks identical to a present one unless
    // the tones are compared against each other.
    for (const [tone] of TONES) {
      expect(seen[tone], `${tone} must resolve to a real colour`).toMatch(/^rgb/);
    }
    expect(seen['success'], 'success and danger must not read the same').not.toBe(seen['danger']);
    expect(seen['warning'], 'warning and danger must not read the same').not.toBe(seen['danger']);
    expect(seen['warning'], 'warning and success must not read the same').not.toBe(seen['success']);

    // Neutral is the only tone that SHOULD read as plain text, so it is
    // asserted against the host's own metric number rather than against a
    // token this test resolves for itself - the point is that the two screens
    // agree, not that one matches a value reconstructed here.
    //
    // Deliberately NOT claiming to guard the missing `.plugin-figure-neutral`
    // rule that prompted all this. `.plugin-figure-value` sets this same
    // colour, so deleting the neutral rule changes nothing a browser can
    // measure and this assertion would keep passing. The Rust test that
    // requires every emitted class to have a rule is what catches that, and it
    // is the only thing that can.
    const hostNumber = await page
      .locator('.metric-value')
      .evaluate((el) => getComputedStyle(el).color);
    expect(hostNumber, 'the host metric must have a colour to compare against').toMatch(/^rgb/);
    expect(
      seen['neutral'],
      'an untoned figure must read exactly like the dashboard\'s own number',
    ).toBe(hostNumber);
    for (const t of ['success', 'warning', 'danger']) {
      expect(seen[t], `${t} must not fall back to the plain text colour`).not.toBe(hostNumber);
    }
  });

  /**
   * The tile is meant to be the same box as the dashboard's own metric card,
   * because on an operator screen it sits directly beside four of them.
   *
   * It was not: no background and no shadow where the card has both, and 12px
   * of padding against the card's 20px. The radius came from `--radius-md`,
   * which no :root in this stylesheet defines, so it fell through to its own
   * hardcoded `8px` - which is also what `--border-radius-lg` resolves to, so
   * that part was latent rather than visible. Asserted here because a `var()`
   * fallback cannot fail loudly: a token that stops existing looks exactly
   * like one that works, until its value and the fallback diverge.
   */
  test('the tile is the same box as a dashboard metric card', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.setContent(`<style>${CSS}</style>${MARKUP}${HOST_CARD}`);

    const box = (sel: string) =>
      page.locator(sel).evaluate((el) => {
        const s = getComputedStyle(el);
        return {
          radius: s.borderTopLeftRadius,
          padding: s.paddingTop,
          border: s.borderTopWidth,
          bg: s.backgroundColor,
        };
      });
    const figure = await box('.plugin-figure-success');
    const metric = await box('.metric-card');

    expect(figure.radius, 'radius must come from the shared token').toBe(metric.radius);
    expect(figure.padding, 'padding must match the host card').toBe(metric.padding);
    expect(figure.border, 'border width must match the host card').toBe(metric.border);
    expect(figure.bg, 'the tile must sit on the same surface').toBe(metric.bg);
    expect(figure.radius, 'a radius of 0 would mean the token did not resolve').not.toBe('0px');

    const read = (sel: string) =>
      page.locator(sel).evaluate((el) => {
        const s = getComputedStyle(el);
        return { size: s.fontSize, weight: s.fontWeight };
      });
    const value = await read('.plugin-figure-success .plugin-figure-value');
    const mValue = await read('.metric-value');
    expect(value.size, 'the number must be as prominent as the host metric').toBe(mValue.size);
    expect(value.weight, 'and as heavy').toBe(mValue.weight);
  });

  /**
   * Four tiles across on a dashboard, and not four tiles across on a phone.
   * The renderer sets the column count from the plugin's own `grid` element,
   * inline - so this asserts the one thing the stylesheet still owns: that a
   * tile is not squeezed to nothing when the grid is narrow.
   */
  test('a tile stays legible at a phone width', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.setContent(`<style>${CSS}</style>${MARKUP}`);
    const w = await page
      .locator('.plugin-figure-success')
      .evaluate((el) => el.getBoundingClientRect().width);
    expect(w, 'a tile must not collapse on a narrow screen').toBeGreaterThan(80);
  });
});
