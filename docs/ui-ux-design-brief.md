# UI/UX Design Brief: EVM PayServer Client

## Overview

The EVM PayServer Client is a merchant-facing payment dashboard for managing stores, invoices, payments, and wallets across multiple EVM blockchains. The application is a single-page WebAssembly application built with Leptos, providing real-time payment status updates via WebSocket and a cohesive merchant experience.

**Key Principle:** One client, any chain. The UI adapts to the merchant's configured payserver without requiring code changes.

---

## Design System

### Design Tokens

All visual decisions are driven by tokens defined in `styles.css` at the `:root` level, maintaining consistency across the interface:

- **Colors**: Primary indigo (`--color-primary: #635bff`), semantic status colors (green success, amber warning, red error, blue info), and neutral grays
- **Spacing**: Configurable space tokens (`--space-*`) for margins and padding
- **Typography**: Inter font family, with weight variants for hierarchy (400 regular, 500 medium, 600 semibold, 700 bold)
- **Borders**: Rounded corners (`--border-radius-sm`, `md`, `lg`) and consistent border colors
- **Shadows**: Subtle elevation for layering

### Component Vocabulary

- **`.ps-page`**: Main page container—a column layout with consistent gap and max-width for content
- **`.page-header-row`**: Flex row pairing page title and status/actions (space-between)
- **`.ps-card`**: Standard card container with clipped children, used throughout
- **`.ps-card-header`**: Header within a card—flex row for title + status or actions
- **`.btn` / `.ps-btn`**: Button element with variants
  - Primary (indigo, clickable primary actions)
  - Secondary / Ghost (light background or outlined)
  - Danger (red, destructive actions)
  - Success (green, confirmatory actions)
  - Warning (amber, caution states)
  - Sizes: sm, md (default), lg
- **Status Components**: `LoadingState`, `LoadingInline`, `ErrorState`, `EmptyState` (never hand-rolled divs)
- **Feedback**: Pagination, pagination controls, real-time status banners

---

## Flow: Authentication

**Routes**: `/login`, `/register`, `/recovery`

### Login (`/login`)
- Unauthenticated entry point for merchant access
- Username/identifier and WebAuthn (passkey) authentication
- Footer link to recovery page for lost access
- Redirect to dashboard or prior page on success

### Register (`/register`)
- New merchant account creation
- Collects identifier, email, and WebAuthn passkey enrollment
- Generates and displays a BIP39 recovery phrase (stored client-side only)
- CAPTCHA protection for abuse prevention
- Navigates to dashboard on successful registration

### Recovery (`/recovery`)
- Unauthenticated account recovery without passkey access
- Step 1: Enter account identifier
- Step 2: Enter full BIP39 recovery phrase
- Server verifies phrase hash; returns wrapped key
- Client unwraps locally, creates new passkey (WebAuthn)
- Step 3: Displays newly generated recovery phrase
- Revokes all other passkeys and sessions on completion
- All crypto operations are client-side only (phrase never leaves browser)

---

## Flow: Checkout

**Route**: `/checkout/:id`

### Public Checkout Page
- Unauthenticated customer-facing payment interface
- No authentication required; accessible by any visitor
- **Key UI Elements**:
  - Invoice amount and description
  - QR code picker for payment (chain and asset selection)
  - Countdown timer showing invoice expiry
  - Real-time payment status via WebSocket
  - Wallet connection options (WalletConnect, Coinbase Wallet)
  - Manual copy-to-clipboard for address and amount

### Payment Status Flow
- **Pending**: Invoice awaiting payment
- **Paid**: Invoice has received sufficient funds (displays immediately on WebSocket update)
- **Expired**: Countdown reaches zero; timer halts
- Polls server every 10 seconds if WebSocket unavailable; refreshes on WS payment update

---

## Flow: Dashboard

**Route**: `/` (authenticated only)

### Dashboard Home
- Primary authenticated landing page
- **Sections**:
  - **Onboarding Checklist**: Guides new merchants through setup (store creation, wallet link, first invoice)
  - **Dashboard Header**: Time window selector (7D, 30D, 90D), export analytics button
  - **Metrics Cards**: Key stats (total volume, payment count, average transaction size)
  - **Charts**: Analytics visualizations over selected time period
  - **Recent Activity**: Log of invoices and payments with real-time WebSocket updates
  - **Network Status**: Chain connection health and status

### Real-Time Updates
- WebSocket subscriptions push invoice status and payment updates
- Dashboard metrics and activity log refresh on update receipt
- No polling when WebSocket is active

---

## Flow: Store Management

**Routes**: `/evm/stores`, `/evm/stores/:id`

### Stores List (`/evm/stores`)
- Table and card views of all merchant stores
- **Columns/Fields**: Store name, chain(s) supported, status, payment methods, created date
- **Actions**: Create store (+ button), edit, view settings
- **Filtering**: By status, chain, or store type
- **Empty State**: Onboarding prompt if no stores exist

### Store Detail Page (`/evm/stores/:id`)
- Read and configure a single store
- **Tabs**:
  - **Settings**: Store name, description, webhook URL, plugin configuration
  - **Payment Methods**: Asset selection, custom pricing tiers
  - **Branding**: Logo, colors (if applicable)
  - **API Keys**: Management and scoping
- **Actions**: Save changes, delete store, view invoices for this store
- Status indicator in page header (active/inactive/configuration needed)

---

## Flow: Invoices

**Routes**: `/evm/invoices`, `/evm/invoices/:id`

### Invoices List (`/evm/invoices`)
- Primary merchant workflow—track all payment requests
- **Columns**: Amount, currency, invoice ID, customer reference, status, expiry, payment date
- **Status Indicators**: Pending (yellow), Paid (green), Expired (gray), Failed (red)
- **Controls**:
  - Filter by status (All, Pending, Paid, Expired)
  - Filter by currency
  - Search by invoice ID or customer reference
  - TX hash lookup (finds invoice by blockchain transaction hash)
  - Create Invoice button
  - Export list to CSV
- **Pagination**: Offset-based with page size 25
- **Real-Time Updates**: WebSocket pushes update invoice status; list refreshes without navigation
- **Empty State**: Displays when no stores selected or no invoices exist

### Create Invoice Modal
- Quick-add modal for new invoices
- Fields: Amount, asset/currency, customer name, description, expiry time
- Calculates fiat equivalent if applicable
- On success, shows new invoice ID and navigates to detail page

### Invoice Detail Page (`/evm/invoices/:id`)
- Comprehensive invoice view
- **Display**:
  - Invoice ID and QR code (links to public checkout)
  - Amount, currency, created/expiry timestamps
  - Customer name and reference
  - Payment status and real-time updates
  - Blockchain transaction hash (if paid)
  - Payment timeline and receipt
- **Actions**: Copy public link, print invoice, refund (if applicable)
- **Status Tracking**: Shows every payment attempt and final confirmation

---

## Flow: Payments

**Routes**: `/evm/payments`, `/evm/payments/:id`

### Payments List (`/evm/payments`)
- Settled payment history across all invoices
- **Columns**: Amount, currency, invoice ID, transaction hash, confirmation status, payment date
- **Controls**:
  - Filter by currency
  - Filter by confirmation status (pending, confirmed, failed)
  - Search by transaction hash
  - Export to CSV
- **Pagination**: Offset-based
- **Empty State**: Guides to create an invoice if none exist

### Payment Detail Page (`/evm/payments/:id`)
- Full blockchain transaction view
- **Display**:
  - Transaction hash (clickable to block explorer)
  - From/to addresses
  - Amount and confirmation count
  - Gas fee and total cost
  - Timestamp and block number
  - Invoice it pays and merchant reference
- **Status**: Pending confirmations, confirmed, or failed
- **Actions**: Copy hash, link to block explorer

---

## Flow: Wallets

**Routes**: `/evm/wallets`, `/evm/wallets/:id`

### Wallets List (`/evm/wallets`)
- Merchant's account wallets linked to payment processing
- **Columns**: Wallet name, address (abbreviated), balance, chain, status, linked date
- **Actions**: Add wallet, view details, revoke access
- **Filtering**: By chain or status
- **Empty State**: Guides to link a wallet to start accepting payments

### Wallet Detail Page (`/evm/wallets/:id`)
- Configure and monitor a wallet
- **Display**:
  - Full wallet address with copy button
  - Current balance across all linked stores
  - Chain and network information
  - Webhook endpoint configuration
  - Recent transactions from this wallet
- **Actions**: Rotate API key, unlink wallet, view all transactions
- **Status**: Active, pending confirmation, or revoked

---

## Flow: Settings

**Route**: `/evm/settings` (multi-tab interface)

### Account Settings
- Merchant profile and preferences
- **Fields**: Display name, email, timezone, notification preferences
- **Actions**: Update profile, change password (security)

### Notifications Settings
- Control real-time alerts
- **Options**: 
  - Payment received notifications
  - Failed payment alerts
  - Invoice expiry warnings
  - System announcements
- **Channels**: Email, in-app banner, webhook
- **Delivery Schedule**: Real-time or batch digests

### Admin/Organization Settings
- Team and permission management
- **Controls**:
  - Add/remove team members
  - Assign roles (owner, admin, operator)
  - Audit logs (recent changes)
  - Session management (active devices)
- **Actions**: Revoke sessions, reset API credentials

### Security Settings
- Passkey and authentication management
- **Options**:
  - Manage registered passkeys (add, rename, revoke)
  - Add new device (secondary passkey enrollment)
  - View login history
  - Revoke all sessions

### Plugins (`/evm/plugins/:id/:path`)
- Merchant-installed plugins extend dashboard functionality
- **Dynamic Routes**: Each plugin defines its own path namespace
- **Architecture**: Plugins are compiled Rust components, not runtime JavaScript
  - Plugin markup must use `class="ps-"` prefixed classes (all styling enforced)
  - Plugin pages inherit `.ps-page` layout and design token access
- **Extensibility**: Plugin can request store context, invoice data, or custom API endpoints
- **Isolation**: Each plugin runs in the same WASM bundle but cannot access other plugin state

---

## Design Conventions

### Page Layout
- All authenticated pages use `.ps-page` container (column, gap, max-width)
- Page header always includes `.page-header-row` with title and optional status/actions
- Pages stack sections vertically with consistent spacing

### Data Display
- **Tables**: Used for high-density tabular data (invoices, payments, transactions)
- **Cards**: Grouped related information (store details, wallet summary, metrics)
- **Status Indicators**: Always rendered as colored badges or text (green, yellow, red, blue per semantic color)
- **Timestamps**: Localized to user timezone; relative time for recent events (e.g., "2 hours ago")

### Forms
- Input fields use `.ps-label` and consistent padding
- Required fields marked with asterisk (*)
- Error messages inline and below field (red text)
- Submit button disabled during processing
- Confirmation dialogs for destructive actions (delete store, revoke wallet)

### Feedback States
- **Loading**: `LoadingState` (spinner + placeholder) or `LoadingInline` (compact spinner)
- **Error**: `ErrorState` (icon + message + retry button)
- **Empty**: `EmptyState` (illustration + message + primary action)
- **Success**: Toast or banner notification (green, auto-dismiss after 5 seconds)
- **Warning**: Banner or modal for caution actions (amber background)

### Real-Time Updates
- WebSocket-driven updates use optimistic UI (show update immediately, validate server response)
- Stale data never overwrites fresh: only update if resource version is newer
- Disconnection shows transparent overlay with "Reconnecting..." message
- Failed updates surface error toast with retry button

### Mobile Responsiveness
- `.ps-page` max-width adapts to viewport (full-width on mobile, constrained on desktop)
- Tables convert to card view on mobile (stack rows as cards, swipe for extra columns)
- Forms stack vertically on mobile
- Navigation collapses to hamburger menu below 768px

### Accessibility
- All buttons and interactive elements are keyboard-navigable (Tab, Enter, Space)
- Color not the only indicator of status (text labels + icons)
- Forms have associated labels for all inputs
- Error messages linked to fields with `aria-describedby`
- Modal overlays trap focus and prevent background scrolling

### Keyboard Shortcuts
- `/` to open search (invoice, payment, transaction lookup)
- `?` to open help/keyboard shortcut reference
- `Escape` to close modals and overlays
- `Ctrl+Enter` to submit forms (standard)

---

## Responsive Behavior

### Desktop (>768px)
- Three-column layout: sidebar navigation, main content, sidebar panels
- Data tables display all columns
- Forms display inline validation
- Hover effects on interactive elements

### Tablet (480px–768px)
- Two-column layout: collapsed sidebar + main content
- Table columns reorder by importance; less critical columns hidden
- Bottom navigation for main flows

### Mobile (<480px)
- Single-column full-screen layout
- Hamburger navigation
- All interactions use tap (no hover)
- Forms one-field-per-screen in modals
- Bottom action buttons (easier thumb reach)

---

## Summary

The EVM PayServer Client provides a cohesive, merchant-focused interface for managing blockchain payments. It emphasizes clarity through design tokens, real-time feedback via WebSocket, and a clear information hierarchy. Every flow—from onboarding through daily operations—is optimized for both new and power-user merchants, with extensibility for plugins and customization without requiring code changes to the core client.
