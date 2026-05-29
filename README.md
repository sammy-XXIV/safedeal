# SafeDeal

Trustless escrow agent for the Vara A2A Network — deposit, confirm or auto-refund on timeout.

Built for the Vara Agents Arena Season 1 hackathon (Economy & Markets track).

## Overview

SafeDeal is a smart contract implementing a two-party escrow mechanism on Vara Network. It enables secure transactions between a buyer and seller by holding funds in escrow until delivery is confirmed or automatically refunding on timeout.

### Key Features

- **Secure Escrow**: Buyer deposits VARA while seller awaits confirmation
- **Instant Settlement**: Once buyer confirms delivery, funds are instantly released to seller
- **Auto-Refund**: If delivery is not confirmed within the timeout period, funds automatically refund to buyer
- **Flat Fee Model**: Simple 1 VARA flat fee per escrow transaction
- **Transparent State**: Query escrows by buyer, seller, or ID
- **Gas Efficient**: Optimized for minimal gas consumption on Vara

## How It Works

### 1. Create Escrow
Buyer initiates an escrow transaction:
- Deposits VARA amount + 1 VARA fee
- Specifies seller address
- Sets timeout duration (in blocks)
- Receives escrow_id as reference

### 2. Confirm Delivery (Happy Path)
Once goods/services are delivered:
- Buyer calls `confirm_delivery(escrow_id)`
- Funds are instantly released to seller
- Escrow marked as completed
- 1 VARA fee is retained (covers transaction costs)

### 3. Auto-Refund (Timeout Path)
If confirmation doesn't happen before timeout:
- Anyone can trigger `refund_on_timeout(escrow_id)`
- Funds automatically return to buyer
- Escrow marked as refunded

## API Reference

### Messages

#### `create_escrow(seller: ActorId, timeout_blocks: u32) -> Result<u64, SafeDealError>`

Creates a new escrow. Caller becomes the buyer.

**Parameters:**
- `seller`: Actor ID of the seller receiving the funds
- `timeout_blocks`: Number of blocks before auto-refund triggers

**Returns:** `escrow_id` on success

**Attached Value:** Must send buyer's deposit amount + 1 VARA fee

---

#### `confirm_delivery(escrow_id: u64) -> Result<(), SafeDealError>`

Confirms delivery and releases funds to seller.

**Parameters:**
- `escrow_id`: ID of the escrow to confirm

**Caller:** Must be the buyer who created the escrow

**Returns:** () on success

---

#### `refund_on_timeout(escrow_id: u64) -> Result<(), SafeDealError>`

Triggers auto-refund for a timed-out escrow.

**Parameters:**
- `escrow_id`: ID of the escrow to refund

**Precondition:** Current block height must be >= timeout_at

**Returns:** () on success

---

#### `get_escrow(escrow_id: u64) -> Option<Escrow>`

Queries details of a specific escrow.

**Parameters:**
- `escrow_id`: ID of the escrow

**Returns:** Escrow details or None if not found

---

#### `get_escrows_by_buyer(buyer: ActorId) -> Vec<Escrow>`

Lists all escrows where the given actor is the buyer.

---

#### `get_escrows_by_seller(seller: ActorId) -> Vec<Escrow>`

Lists all escrows where the given actor is the seller.

---

#### `get_fee() -> u128`

Returns the flat fee amount in smallest VARA units (1 VARA = 10^12 units).

## Data Structures

### Escrow

```rust
pub struct Escrow {
    pub escrow_id: u64,           // Unique identifier
    pub buyer: ActorId,           // Buyer address
    pub seller: ActorId,          // Seller address
    pub amount: u128,             // Amount in escrow (excluding fee)
    pub timeout_at: u32,          // Block height for timeout
    pub status: EscrowStatus,     // Pending, Confirmed, Completed, Refunded
    pub created_at: u32,          // Creation block height
}
```

### EscrowStatus

```rust
pub enum EscrowStatus {
    Pending,      // Awaiting confirmation or timeout
    Confirmed,    // Buyer confirmed, releasing funds
    Completed,    // Funds released to seller
    Refunded,     // Auto-refund triggered on timeout
}
```

### SafeDealError

```rust
pub enum SafeDealError {
    InsufficientFunds,        // Deposit < amount + fee
    InvalidEscrowId,          // Escrow not found
    InvalidStatus,            // Wrong status for operation
    UnauthorizedActor,        // Only buyer/seller can call
    EscrowAlreadyCompleted,   // Cannot modify completed escrow
    TimeoutNotReached,        // Timeout not yet triggered
    SellerCannotBeZero,       // Seller must be non-zero address
}
```

## Building

### Prerequisites
- Rust 1.70+
- Vara SDK/Sails framework

### Compile

```bash
cargo build --release
```

The compiled WASM binary will be in `target/release/safe_deal.wasm`

### Test

```bash
cargo test
```

## Example Usage

```rust
// Buyer creates escrow for 10 VARA (+ 1 VARA fee) with 1000 block timeout
let escrow_id = safe_deal.create_escrow(seller_id, 1000).await?;

// Seller waits for delivery...

// Buyer confirms delivery (happy path)
safe_deal.confirm_delivery(escrow_id).await?;
// -> Seller receives 10 VARA
// -> 1 VARA fee is retained

// OR if no confirmation within 1000 blocks:
// Anyone can refund
safe_deal.refund_on_timeout(escrow_id).await?;
// -> Buyer receives 10 VARA back
```

## Constants

- **ESCROW_FEE**: 1 VARA (1,000,000,000,000 in smallest units)

## Security Considerations

- **Actor Authentication**: Only the buyer can confirm delivery; sender is authenticated via `msg::source()`
- **Status Transitions**: Strict state machine prevents double-spending and invalid operations
- **Value Safety**: Fund transfers verified before state updates
- **Timeout Validation**: Refunds only trigger after timeout block height is reached

## Architecture Decisions

1. **Sails Framework**: Async-first messaging for clean, composable contracts
2. **Flat Fee Model**: Simpler UX than percentage fees; easier to predict transaction costs
3. **Auto-Refund Design**: Timeout can be triggered by any actor, making refunds censorship-resistant
4. **CollectionsMap**: Efficient storage for escrow state with fast lookups by ID

## License

MIT or Apache-2.0

## Hackathon Track

Vara Agents Arena Season 1 — Economy & Markets Track
