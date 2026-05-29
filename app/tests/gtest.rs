use safedeal_app::{Program, Escrow, EscrowStatus, SafeDealError};
use sails_rs::gtest;

#[test]
fn test_create_escrow_happy_path() {
    let mut system = gtest::System::new();
    let mut program = Program::new();

    let buyer = gtest::ActorId::new(1);
    let seller = gtest::ActorId::new(2);
    let fee = 1_000_000_000_000u128;
    let amount = 5_000_000_000_000u128;

    let before = system.balance_of(&buyer);

    let result = program
        .safedeal()
        .create_escrow(seller, 100)
        .with_actor_id(buyer)
        .with_value(fee + amount)
        .call(&mut system);

    let after = system.balance_of(&buyer);
    let spent = before - after;

    assert_eq!(spent, fee + amount, "Buyer should pay fee + amount");
    assert!(result.is_ok(), "Create escrow should succeed");
}

#[test]
fn test_create_escrow_insufficient_funds() {
    let mut system = gtest::System::new();
    let mut program = Program::new();

    let buyer = gtest::ActorId::new(1);
    let seller = gtest::ActorId::new(2);
    let insufficient = 500_000_000_000u128; // Less than fee

    let result = program
        .safedeal()
        .create_escrow(seller, 100)
        .with_actor_id(buyer)
        .with_value(insufficient)
        .call(&mut system);

    assert!(matches!(result, Err(SafeDealError::InsufficientFunds)),
        "Should reject insufficient payment");
}

#[test]
fn test_create_escrow_zero_seller() {
    let mut system = gtest::System::new();
    let mut program = Program::new();

    let buyer = gtest::ActorId::new(1);
    let seller = gtest::ActorId::zero();
    let fee = 1_000_000_000_000u128;

    let result = program
        .safedeal()
        .create_escrow(seller, 100)
        .with_actor_id(buyer)
        .with_value(fee)
        .call(&mut system);

    assert!(matches!(result, Err(SafeDealError::SellerCannotBeZero)),
        "Should reject zero seller address");
}

#[test]
fn test_confirm_delivery_happy_path() {
    let mut system = gtest::System::new();
    let mut program = Program::new();

    let buyer = gtest::ActorId::new(1);
    let seller = gtest::ActorId::new(2);
    let fee = 1_000_000_000_000u128;
    let amount = 5_000_000_000_000u128;

    // Create escrow
    let escrow_id = program
        .safedeal()
        .create_escrow(seller, 100)
        .with_actor_id(buyer)
        .with_value(fee + amount)
        .call(&mut system)
        .expect("Should create escrow");

    let seller_before = system.balance_of(&seller);

    // Confirm delivery
    let result = program
        .safedeal()
        .confirm_delivery(escrow_id)
        .with_actor_id(buyer)
        .call(&mut system);

    let seller_after = system.balance_of(&seller);
    let received = seller_after - seller_before;

    assert!(result.is_ok(), "Confirm should succeed");
    assert_eq!(received, amount, "Seller should receive escrow amount");
}

#[test]
fn test_confirm_delivery_unauthorized() {
    let mut system = gtest::System::new();
    let mut program = Program::new();

    let buyer = gtest::ActorId::new(1);
    let seller = gtest::ActorId::new(2);
    let attacker = gtest::ActorId::new(3);
    let fee = 1_000_000_000_000u128;
    let amount = 5_000_000_000_000u128;

    // Create escrow
    let escrow_id = program
        .safedeal()
        .create_escrow(seller, 100)
        .with_actor_id(buyer)
        .with_value(fee + amount)
        .call(&mut system)
        .expect("Should create escrow");

    // Try to confirm as attacker
    let result = program
        .safedeal()
        .confirm_delivery(escrow_id)
        .with_actor_id(attacker)
        .call(&mut system);

    assert!(matches!(result, Err(SafeDealError::UnauthorizedActor)),
        "Only buyer should be able to confirm");
}

#[test]
fn test_refund_on_timeout_happy_path() {
    let mut system = gtest::System::new();
    let mut program = Program::new();

    let buyer = gtest::ActorId::new(1);
    let seller = gtest::ActorId::new(2);
    let fee = 1_000_000_000_000u128;
    let amount = 5_000_000_000_000u128;

    // Create escrow with 50 block timeout
    let escrow_id = program
        .safedeal()
        .create_escrow(seller, 50)
        .with_actor_id(buyer)
        .with_value(fee + amount)
        .call(&mut system)
        .expect("Should create escrow");

    let buyer_before = system.balance_of(&buyer);

    // Advance blocks past timeout
    system.advance_blocks(100);

    // Refund on timeout
    let result = program
        .safedeal()
        .refund_on_timeout(escrow_id)
        .with_actor_id(buyer)
        .call(&mut system);

    let buyer_after = system.balance_of(&buyer);
    let refunded = buyer_after - buyer_before;

    assert!(result.is_ok(), "Refund should succeed");
    assert_eq!(refunded, amount, "Buyer should receive refund amount");
}

#[test]
fn test_refund_before_timeout_fails() {
    let mut system = gtest::System::new();
    let mut program = Program::new();

    let buyer = gtest::ActorId::new(1);
    let seller = gtest::ActorId::new(2);
    let fee = 1_000_000_000_000u128;
    let amount = 5_000_000_000_000u128;

    // Create escrow with 100 block timeout
    let escrow_id = program
        .safedeal()
        .create_escrow(seller, 100)
        .with_actor_id(buyer)
        .with_value(fee + amount)
        .call(&mut system)
        .expect("Should create escrow");

    // Try to refund before timeout (only 10 blocks passed)
    system.advance_blocks(10);

    let result = program
        .safedeal()
        .refund_on_timeout(escrow_id)
        .with_actor_id(buyer)
        .call(&mut system);

    assert!(matches!(result, Err(SafeDealError::TimeoutNotReached)),
        "Should reject refund before timeout");
}

#[test]
fn test_get_escrow() {
    let mut system = gtest::System::new();
    let mut program = Program::new();

    let buyer = gtest::ActorId::new(1);
    let seller = gtest::ActorId::new(2);
    let fee = 1_000_000_000_000u128;
    let amount = 5_000_000_000_000u128;
    let timeout = 100u32;

    let escrow_id = program
        .safedeal()
        .create_escrow(seller, timeout)
        .with_actor_id(buyer)
        .with_value(fee + amount)
        .call(&mut system)
        .expect("Should create escrow");

    let escrow = program
        .safedeal()
        .get_escrow(escrow_id)
        .call(&mut system)
        .expect("Should get escrow");

    assert_eq!(escrow.escrow_id, escrow_id);
    assert_eq!(escrow.buyer, buyer);
    assert_eq!(escrow.seller, seller);
    assert_eq!(escrow.amount, amount);
    assert_eq!(escrow.status, EscrowStatus::Pending);
}

#[test]
fn test_get_fee() {
    let mut system = gtest::System::new();
    let program = Program::new();

    let fee = program
        .safedeal()
        .get_fee()
        .call(&mut system);

    assert_eq!(fee, 1_000_000_000_000);
}
