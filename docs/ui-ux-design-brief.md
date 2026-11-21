# UI/UX design brief

This describes the screens a merchant and their customer see today, and the
design conventions the client already uses. It is a description of the
existing UI, not a redesign. Where the code and a convention disagree, this
brief says so.

The client is a Leptos app compiled to WebAssembly and served by nginx. It
talks to whichever payserver is configured at runtime.

## Who uses it

- **Merchant.** Signs in, sets up stores and wallets, creates and tracks
  invoices, and reads payments. Everything under `/evm` and `/`.
- **Customer.** Pays one invoice from a public checkout link. No account, no
  navigation. Only `/checkout/:id`.

## Route inventory

The router (`src/app/mod.rs`) and `e2e-routes.txt` agree on these routes.

| Route | Screen | Auth |
| --- | --- | --- |
| `/login` | Sign-in (ui-kit `LoginPage`), with a "Recover it" link | public |
| `/register` | Account registration (ui-kit `RegisterPage`) | public |
| `/recovery` | Recover an account from a recovery phrase | public |
| `/checkout/:id` | Customer checkout for one invoice | public |
| `/` | Dashboard (same page as `/evm`) | protected |
| `/evm` | Dashboard | protected |
| `/evm/invoices` | Invoice list | protected |
| `/evm/invoices/:id` | Invoice detail | protected |
| `/evm/payments` | Payment list | protected |
| `/evm/payments/:id` | Payment detail | protected |
| `/evm/stores` | Store list | protected |
| `/evm/stores/:id` | Store detail (tabs) | protected |
| `/evm/wallets` | Wallet list | protected |
| `/evm/wallets/:id` | Wallet detail (tabs) | protected |
| `/evm/settings` | Settings (tabs) | protected |
| `/evm/plugins/:id/:path` | A plugin's page, as the server declares it | protected |

Any other path renders "Page Not Found" with a link back to the dashboard
(`src/pages/not_found.rs`).

Protected routes share one layout (`ProtectedLayout`): a sidebar, a header,
and the page in an outlet.

## Flows

### Sign-in, registration, recovery

- `/login` is ui-kit's `LoginPage`, which this repo does not own. The client
  adds one line under it: "Lost access to your account? Recover it", linking
  to `/recovery`. It passes a `redirect_to` taken from the return-to store, so
  a user sent to sign in returns to where they were.
- `/recovery` asks for an account identifier (email, wallet address, or
  account id) and the full recovery phrase. It then registers a new passkey
  for the account. Its states are: the form, "Recovering your account..."
  with an inline spinner, and "Account recovered".

### Dashboard and onboarding

- The sidebar's first item is Dashboard. Below the metrics, a checklist walks
  a new merchant through three steps: **Create a store**, **Add a wallet**,
  **Create your first invoice**. The checklist disappears once all three are
  done.
- A store with no wallet cannot take payments, so "Add a wallet" is part of
  the gate, not a tour.
- An optional fourth row, "Add a contact email", is never counted in the
  three. It can be dismissed with "Not now"; the dismissal is kept in the
  browser only.
- Dashboard panels include charts, recent activity, and network status.

### Stores

- List page: title "Stores", a "Create store" primary button that toggles an
  inline create form (its submit reads "Create", then "Creating..." while in
  flight), and a "Show archived" toggle.
- Detail page: tabs **General**, **Payment Methods**, **Webhooks**,
  **Settings**. Archived stores carry an "Archived" badge; the action is
  "Archive".
- Webhook tab actions: "Configure webhook" when none exists, otherwise "Edit
  endpoint" and "Regenerate secret".

### Wallets

- List page: title "Wallets", with a primary "Add wallet" button in the page header. It toggles to "Cancel" while the form is open.
- Detail page: tabs **General** and **Addresses**. Export shows "Exporting..."
  while it runs. A wallet with no name shows "Unnamed Wallet".

### Invoices

- Header button "Create Invoice" opens a modal (available from every
  protected page). The modal's submit reads "Create Invoice", then
  "Creating...". With no wallet it offers "Add a wallet".
- List page: title "Invoices", subtitle "Create and manage payment
  invoices", a "Create invoice" button, a status filter (All statuses,
  Pending, Processing, Partially Paid, Paid, Expired, Cancelled, Refunded,
  Late Paid), a store filter, search, CSV export, and pagination. The empty
  state reads "Create an invoice to get started, or adjust your filters."
- The list updates live: WebSocket status changes patch the row in place
  without a full refetch.
- Detail page shows the invoice's status badge, its metadata and payments.
  Its summary rows include a Paid and an Expired flag, each shown as Yes or
  No. A paid invoice adds an "Invoice paid in full" step to its timeline.

### Payments

- List page: title "Payments", filterable and paginated, status badges per
  row.
- Detail page: a status block. A payment that is not yet final shows a
  warning or info line explaining what is still pending.

### Checkout (customer)

Public, no navigation, no sign-in. The page is branded "ETHPayServer" with
"Powered by ETHPayServer" in the footer.

- Loading: "Loading checkout...". If the invoice cannot be loaded:
  "Invoice unavailable. Check the link and try again."
- Live state: a "Live" indicator when the WebSocket is connected.
- Pay with: a selector, labelled "Pay with", for the chain and asset.
- Two QR cards, each with its own caption: "Scan with a wallet" and "Copy to
  an exchange withdrawal". The QR picker cycles encodings with previous/next
  buttons and arrow keys, with a labelled tablist of dots.
- Amount, network and address are shown as label/value pairs below.
- Countdown: "Expires in" followed by the time. It stops at "Expired", and it
  also stops once a payment covering the full amount is detected and only
  waits on confirmations.
- Status copy, by state:
  - awaiting payment: "Awaiting payment"
  - processing: "Payment detected, awaiting confirmation..."
  - partially paid: "Partial payment received"
  - refunded: "Payment refunded"
  - complete: "Payment Complete. Thank you for your payment."
  - expired: "Invoice Expired. This invoice is no longer accepting payments."
  - cancelled: "Invoice Cancelled"
  - no active methods: "No Payment Options. This invoice has no active
    payment methods configured."
- Freshness: the page re-reads the invoice on a slow timer
  (about every ten seconds) behind the WebSocket, so a missed socket message
  does not leave a stale countdown.

### Settings

- Title "Settings", subtitle "Manage your account and preferences". Tabs:
  **Account**, **Security**, **Preferences**, **API Keys**, **Notifications**,
  and a **Server Admin** tab. The tab is kept in page state, not in the URL.

### Plugin pages

- A plugin's page renders at `/evm/plugins/:id/:path`. Its link appears in the
  sidebar under "Account", populated from what the server declares. The
  client does not know which plugins exist, and it cannot ship a plugin's UI
  as anything but Rust compiled into the bundle.

## Shell and navigation

- **Sidebar.** Main: Dashboard, Invoices, Payments. "Configuration": Stores,
  Wallets, Settings. Then "Account" plugin links when the server lists any,
  then an external "Documentation" link. A store selector sits at the top,
  with "All Stores", the stores, and "Manage Stores".
- **Header.** "Create Invoice" button, a live connection indicator ("Live",
  "Reconnecting...", or "Offline"), and a user menu with Settings and "Log out".

## Design conventions in use

These are what the code and `styles.css` do today.

### Tokens

- All colours, spacing, type sizes, radii and shadows are CSS custom
  properties on `:root` in `styles.css`. Use `--space-*`, `--text-*`,
  `--border-radius-*`, `--shadow-*`, and the palette. Hard-coded values in new
  rules are the exception.
- Primary is indigo (`--color-primary`). Success, warning, error and info have
  their own hues and light backgrounds. Indigo is kept distinct from the
  semantic colours so that an action never reads as a status.
- The font is Inter, loaded from Google Fonts.

### Components

- **Buttons.** `ps-btn` with a variant (`ps-btn-primary`, `ps-btn-secondary`)
  and `ps-btn-sm` for header and toolbar actions. Legacy `.btn` is aliased to
  `ps-btn`.
- **Status badges.** `badge` with `badge-success`, `badge-warning`,
  `badge-error`, `badge-info` or `badge-neutral`. Payments map confirmed to
  success, confirming to warning, and reorged to error. Invoice statuses use
  the `css_class()` of the status type, which is defined outside this repo.
- **Feedback.** `LoadingState` (full block), `LoadingInline` (in a line),
  `ErrorState`, `EmptyState` and `NoStoreSelected` in
  `src/components/feedback.rs`. Use them instead of hand-rolled divs. List
  pages, charts, plugin pages, and the notifications and security settings
  already do.
- **Store loading.** The store context reports `Loading`, `Loaded` or
  `Failed`, not just an empty list. Onboarding and empty states wait for
  `Loaded`, so a merchant with stores is never told to create one while the
  list is still in flight, and a failed fetch is not mistaken for a new
  account.
- **Cards.** `ps-card` clips its children; `ps-card-header` lays out the
  title and any status on one row. Put status on the header row, opposite the
  title.
- **Inline icons.** Each area keeps its own icon module as inline SVG
  (`app/icons.rs`, `pages/*/icons.rs`). No icon font.

### Page shell

- Pages open with `page-header-row`, `page-title`, an optional
  `page-description`, and `page-actions` for buttons. This is the shape on the
  list and settings pages.
- The `ps-page` shell (column layout, `--space-5` gap, `--content-max-width`)
  is what `styles.css` defines for new pages. In the code today only the
  plugin page uses it; the list pages spell the layout out in the unprefixed
  classes above. Prefer `ps-page` for new work.

### Copy

- Sentence case for labels and headings ("Create store", "Page Not Found").
- Pending labels end in an ellipsis and replace the action: "Creating...",
  "Exporting...", "Recovering your account...".
- Empty states say what to do next, not just that there is nothing.
- Error copy on checkout is plain and actionable: "Check the link and try
  again."

### Responsive and motion

- Breakpoints in `styles.css`: 600px (most layout changes), 640px, 768px,
  900px and 1100px.
- `prefers-reduced-motion: reduce` is honoured.
- There is no dark theme. `styles.css` has no `prefers-color-scheme` or
  `data-theme` rules.

### Accessibility

- The QR previous and next buttons carry `aria-label`, and the QR encoding
  picker is a `tablist` that responds to the arrow keys.
- The onboarding action carries `aria-label` as well.

## Why these conventions

- **Tokens over values.** One palette and one spacing scale keep the screens
  consistent across chains and plugins, and make a theme change a one-file
  edit.
- **Class names are checked.** CI runs `scripts/check-classes-styled.py` and
  `scripts/check-class-roles.py`. A rename once left 104 buttons unstyled on
  testnet, and no test caught it because every class was present, just
  without rules. Each emitted class must have a rule, and a class must not
  serve both a control and a container.
- **Compiled UI.** Every screen is Rust in the bundle. A plugin cannot add
  a page by shipping a template; it declares a page and the client renders its
  own components.
- **Checkout is minimal.** The customer has no navigation, so the page carries
  only the amount, the address, the QR codes and the countdown. Status copy
  says what to do next.
- **Onboarding gates on what blocks payment.** Only the three steps that
  unblock an invoice are required. Optional steps never hold the checklist
  open.

## Known gaps

- The onboarding "Add a contact email" action links to `/settings/account`.
  That is not a route. Settings keeps its tab in page state, so the link lands
  on "Page Not Found". The link should point at `/evm/settings`, and the
  Account tab is the one to show.
- Two pages still spell out the page shell themselves instead of using
  `ps-page`.
